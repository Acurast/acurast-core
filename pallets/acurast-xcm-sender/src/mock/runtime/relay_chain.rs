use frame_support::{
    construct_runtime, parameter_types,
    sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32},
    traits::{Everything, Nothing},
};
use polkadot_runtime_parachains::{configuration, shared, ump};
use sp_core::H256;
use xcm::latest::prelude::*;
use xcm_builder::{
    AllowUnpaidExecutionFrom, FixedRateOfFungible, FixedWeightBounds, LocationInverter,
    SignedToAccountId32,
};
use xcm_executor::{Config, XcmExecutor};

pub type AccountId = AccountId32;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
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
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = Everything;
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl shared::Config for Runtime {}

impl configuration::Config for Runtime {
    type WeightInfo = configuration::TestWeightInfo;
}

parameter_types! {
    pub const KsmLocation: MultiLocation = Here.into();
    pub const KusamaNetwork: NetworkId = NetworkId::Kusama;
    pub const AnyNetwork: NetworkId = NetworkId::Any;
    pub Ancestry: MultiLocation = Here.into();
}

parameter_types! {
    pub const BaseXcmWeight: u64 = 1_000;
    pub KsmPerSecond: (AssetId, u128) = (Concrete(KsmLocation::get()), 1);
    pub const MaxInstructions: u32 = 100;
}

pub type XcmRouter = crate::tests::RelayChainXcmRouter;
pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

pub struct XcmConfig;
impl Config for XcmConfig {
    type Call = Call;
    type XcmSender = XcmRouter;
    type AssetTransactor = ();
    type OriginConverter = ();
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

pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, KusamaNetwork>;

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

parameter_types! {
    pub const FirstMessageFactorPercent: u64 = 100;
}

impl ump::Config for Runtime {
    type Event = Event;
    type UmpSink = ump::XcmSink<XcmExecutor<XcmConfig>, Runtime>;
    type FirstMessageFactorPercent = FirstMessageFactorPercent;
    type ExecuteOverweightOrigin = frame_system::EnsureRoot<AccountId>;
    type WeightInfo = ump::TestWeightInfo;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
        ParasUmp: ump::{Pallet, Call, Storage, Event},
        XcmPallet: pallet_xcm::{Pallet, Call, Storage, Event<T>, Origin},
    }
);
