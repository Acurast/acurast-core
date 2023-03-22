use std::marker::PhantomData;

use acurast_common::Schedule;
use frame_support::traits::OriginTrait;
use pallet_acurast_marketplace::Reward;
use scale_info::TypeInfo;
use sp_core::*;
use sp_std::prelude::*;
use xcm::latest::{Junction, MultiLocation, OriginKind};
use xcm::prelude::*;
use xcm_executor::traits::ConvertOrigin;

pub type AcurastAssetId = AssetId;
pub type InternalAssetId = u32;
pub type AcurastAssetAmount = u128;

use acurast_runtime::AccountId as AcurastAccountId;
use pallet_acurast::{JobModules, JobRegistration, CU32};
use pallet_acurast_marketplace::{
    types::MAX_PRICING_VARIANTS, Advertisement, JobRequirements, PricingVariant, SchedulingWindow,
};

pub const SCRIPT_BYTES: [u8; 53] = hex_literal::hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub fn alice_account_id() -> AcurastAccountId {
    [0; 32].into()
}
pub fn bob_account_id() -> AcurastAccountId {
    [1; 32].into()
}
pub fn owned_asset(amount: u128) -> AcurastAsset {
    AcurastAsset(MultiAsset {
        id: Concrete(MultiLocation {
            parents: 1,
            interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(22)),
        }),
        fun: Fungible(amount),
    })
}
pub fn registration(
) -> JobRegistration<AcurastAccountId, JobRequirements<AcurastAsset, AcurastAccountId>> {
    JobRegistration {
        script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 5000,
            start_time: 1_671_800_400_000, // 23.12.2022 13:00
            end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
            interval: 1_800_000,           // 30min
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: owned_asset(20000),
            min_reputation: None,
            instant_match: None,
        },
    }
}
pub fn asset(id: u32) -> AssetId {
    AssetId::Concrete(MultiLocation::new(
        1,
        X3(
            Parachain(1000),
            PalletInstance(50),
            GeneralIndex(id as u128),
        ),
    ))
}
pub fn advertisement(
    fee_per_millisecond: u128,
) -> Advertisement<AcurastAccountId, AcurastAssetId, AcurastAssetAmount, CU32<10>> {
    let pricing: frame_support::BoundedVec<
        PricingVariant<AcurastAssetId, AcurastAssetAmount>,
        ConstU32<MAX_PRICING_VARIANTS>,
    > = bounded_vec![PricingVariant {
        reward_asset: asset(22),
        fee_per_millisecond,
        fee_per_storage_byte: 0,
        base_fee_per_execution: 0,
        scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
    }];
    Advertisement {
        pricing,
        allowed_consumers: None,
        storage_capacity: 5,
        max_memory: 5000,
        network_request_quota: 8,
        available_modules: JobModules::default(),
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, TypeInfo)]
pub struct AcurastAsset(pub MultiAsset);

impl Reward for AcurastAsset {
    type AssetId = AcurastAssetId;
    type AssetAmount = AcurastAssetAmount;
    type Error = ();

    fn with_amount(&mut self, amount: Self::AssetAmount) -> Result<&Self, Self::Error> {
        self.0 = MultiAsset {
            id: self.0.id.clone(),
            fun: Fungible(amount),
        };
        Ok(self)
    }

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        Ok(self.0.id.clone())
    }

    fn try_get_amount(&self) -> Result<Self::AssetAmount, Self::Error> {
        match self.0.fun {
            Fungible(amount) => Ok(amount),
            _ => Err(()),
        }
    }
}

pub mod acurast_runtime {
    use frame_support::{
        construct_runtime, parameter_types,
        sp_runtime::{testing::Header, traits::AccountIdLookup, AccountId32},
        traits::{AsEnsureOriginWithArg, Everything, Nothing},
        PalletId,
    };
    use pallet_xcm::XcmPassthrough;
    use polkadot_parachain::primitives::Sibling;
    use sp_core::*;
    use sp_runtime::DispatchError;
    use sp_std::prelude::*;
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
        NativeAsset, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::XcmExecutor;

    pub use pallet_acurast::{self, CU32};
    use pallet_acurast_assets_manager::traits::AssetValidator;
    pub use pallet_acurast_marketplace;
    use pallet_acurast_marketplace::{AssetBarrier, AssetRewardManager, JobRequirements};

    use super::{AcurastAsset, AcurastAssetAmount, AcurastAssetId, InternalAssetId};

    pub type AccountId = AccountId32;
    pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;
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
        SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
        SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
        // TODO: safety check of signature
        super::SignedAccountId32FromXcm<RuntimeOrigin>,
        XcmPassthrough<RuntimeOrigin>,
    );

    pub struct AcurastBarrier;

    impl AssetBarrier<AcurastAsset> for AcurastBarrier {
        fn can_use_asset(_asset: &AcurastAsset) -> bool {
            true
        }
    }

    pub struct PassAllAssets {}
    impl<AssetId> AssetValidator<AssetId> for PassAllAssets {
        type Error = DispatchError;

        fn validate(_: &AssetId) -> Result<(), Self::Error> {
            Ok(())
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
            Assets: pallet_assets::{Pallet, Storage, Event<T>, Config<T>}, // hide calls since they get proxied by `pallet_acurast_assets`
            AcurastAssets: pallet_acurast_assets_manager::{Pallet, Storage, Event<T>, Config<T>, Call},
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
        pub const ReportTolerance: u64 = 12000;
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
        type RuntimeCall = RuntimeCall;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = XcmOriginToCallOrigin;
        type IsReserve = NativeAsset;
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
    }

    impl pallet_balances::Config for Runtime {
        type Balance = AcurastAssetAmount;
        type DustRemoval = ();
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeEvent = RuntimeEvent;
        type Balance = AcurastAssetAmount;
        type AssetId = InternalAssetId;
        type AssetIdParameter = codec::Compact<InternalAssetId>;
        type Currency = Balances;
        type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
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
        type RemoveItemsLimit = ();
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = TestBenchmarkHelper;
    }

    impl pallet_acurast_assets_manager::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type ManagerOrigin = frame_system::EnsureRoot<Self::AccountId>;
        type WeightInfo = ();
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = TestBenchmarkHelper;
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub struct TestBenchmarkHelper;
    #[cfg(feature = "runtime-benchmarks")]
    impl pallet_assets::BenchmarkHelper<<Runtime as pallet_assets::Config>::AssetIdParameter>
        for TestBenchmarkHelper
    {
        fn create_asset_id_parameter(
            id: u32,
        ) -> <Runtime as pallet_assets::Config>::AssetIdParameter {
            codec::Compact(id.into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    impl pallet_acurast_assets_manager::benchmarking::BenchmarkHelper<Runtime> for TestBenchmarkHelper {
        fn manager_account() -> <Runtime as frame_system::Config>::AccountId {
            [0; 32].into()
        }
    }

    pub struct FeeManagerImpl;

    impl pallet_acurast_marketplace::FeeManager for FeeManagerImpl {
        fn get_fee_percentage() -> sp_runtime::Percent {
            sp_runtime::Percent::from_percent(30)
        }

        fn get_matcher_percentage() -> sp_runtime::Percent {
            sp_runtime::Percent::from_percent(10)
        }

        fn pallet_id() -> PalletId {
            PalletId(*b"acurfees")
        }
    }

    impl pallet_acurast::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type RegistrationExtra = JobRequirements<AcurastAsset, AccountId>;
        type MaxAllowedSources = frame_support::traits::ConstU32<1000>;
        type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
        type PalletId = AcurastPalletId;
        type RevocationListUpdateBarrier = ();
        type KeyAttestationBarrier = ();
        type UnixTime = pallet_timestamp::Pallet<Runtime>;
        type JobHooks = pallet_acurast_marketplace::Pallet<Runtime>;
        type WeightInfo = pallet_acurast::weights::WeightInfo<Runtime>;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = TestBenchmarkHelper;
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl pallet_acurast::benchmarking::BenchmarkHelper<Runtime> for TestBenchmarkHelper {
        fn registration_extra() -> <Runtime as pallet_acurast::Config>::RegistrationExtra {
            JobRequirements {
                slots: 1,
                reward: AcurastAsset(MultiAsset {
                    id: super::asset(1),
                    fun: Fungible(1),
                }),
                min_reputation: None,
                instant_match: None,
            }
        }
    }

    impl pallet_acurast_marketplace::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type MaxAllowedConsumers = CU32<4>;
        type MaxProposedMatches = frame_support::traits::ConstU32<10>;
        type RegistrationExtra = JobRequirements<AcurastAsset, AccountId>;
        type PalletId = AcurastPalletId;
        type ReportTolerance = ReportTolerance;
        type AssetId = AcurastAssetId;
        type AssetAmount = AcurastAssetAmount;
        type RewardManager = AssetRewardManager<AcurastAsset, AcurastBarrier, FeeManagerImpl>;
        type AssetValidator = PassAllAssets;
        type WeightInfo = pallet_acurast_marketplace::weights::Weights<Runtime>;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = TestBenchmarkHelper;
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl pallet_acurast_marketplace::benchmarking::BenchmarkHelper<Runtime> for TestBenchmarkHelper {
        fn registration_extra(
            r: pallet_acurast_marketplace::JobRequirementsFor<Runtime>,
        ) -> <Runtime as pallet_acurast_marketplace::Config>::RegistrationExtra {
            r
        }
    }

    impl pallet_xcm::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmExecuteFilter = Everything;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Nothing;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl super::mock_msg_queue::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
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
    use sp_core::{ConstU32, H256};
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

    use pallet_acurast::CU32;
    use pallet_acurast_marketplace::JobRequirements;

    use crate::mock::{AcurastAsset, AcurastAssetAmount, AcurastAssetId};

    #[cfg(feature = "runtime-benchmarks")]
    use super::{advertisement, alice_account_id, registration};

    pub type AccountId = AccountId32;
    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );
    pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;
    pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    pub type Block = frame_system::mocking::MockBlock<Runtime>;
    pub type XcmOriginToCallOrigin = (
        SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
        SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
        // TODO: safety check of signature
        super::SignedAccountId32FromXcm<RuntimeOrigin>,
        XcmPassthrough<RuntimeOrigin>,
    );
    pub type LocalAssetTransactor =
        XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, LocationToAccountId, AccountId, ()>;
    pub type XcmRouter = crate::tests::ParachainXcmRouter<MsgQueue>;
    pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

    pub struct XcmConfig;

    impl Config for XcmConfig {
        type RuntimeCall = RuntimeCall;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = XcmOriginToCallOrigin;
        type IsReserve = NativeAsset;
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
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
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeEvent = RuntimeEvent;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    impl super::mock_msg_queue::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type XcmExecutor = XcmExecutor<XcmConfig>;
    }

    impl pallet_xcm::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmExecuteFilter = Everything;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Nothing;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl crate::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type RegistrationExtra = JobRequirements<AcurastAsset, AccountId>;
        type MaxAllowedSources = ConstU32<10>;
        type MaxAllowedConsumers = CU32<10>;
        type AssetId = AcurastAssetId;
        type AssetAmount = AcurastAssetAmount;
        type XcmSender = XcmRouter;
        type AcurastPalletId = AcurastPalletId;
        type AcurastMarketplacePalletId = AcurastMarketplacePalletId;
        type AcurastParachainId = AcurastParachainId;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = BenchmarkHelper;
        type WeightInfo = ();
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub struct BenchmarkHelper;
    #[cfg(feature = "runtime-benchmarks")]
    impl crate::benchmarking::BenchmarkHelper<Runtime> for BenchmarkHelper {
        fn create_job_registration() -> acurast_common::JobRegistration<
            <Runtime as frame_system::Config>::AccountId,
            <Runtime as crate::Config>::RegistrationExtra,
        > {
            registration()
        }

        fn create_allowed_sources_update(
            _index: u32,
        ) -> acurast_common::AllowedSourcesUpdate<<Runtime as frame_system::Config>::AccountId>
        {
            acurast_common::AllowedSourcesUpdate {
                operation: acurast_common::ListUpdateOperation::Add,
                item: alice_account_id(),
            }
        }

        fn create_advertisement() -> pallet_acurast_marketplace::Advertisement<
            <Runtime as frame_system::Config>::AccountId,
            <Runtime as crate::Config>::AssetId,
            <Runtime as crate::Config>::AssetAmount,
            <Runtime as crate::Config>::MaxAllowedConsumers,
        > {
            advertisement(10)
        }
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
        SovereignSignedViaLocation<SovereignAccountOf, RuntimeOrigin>,
        ChildParachainAsNative<origin::Origin, RuntimeOrigin>,
        SignedAccountId32AsNative<KusamaNetwork, RuntimeOrigin>,
        ChildSystemParachainAsSuperuser<ParaId, RuntimeOrigin>,
    );
    pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, KusamaNetwork>;
    pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    pub type Block = frame_system::mocking::MockBlock<Runtime>;
    pub type XcmRouter = crate::tests::RelayChainXcmRouter;
    pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

    pub struct XcmConfig;

    impl Config for XcmConfig {
        type RuntimeCall = RuntimeCall;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = LocalOriginConverter;
        type IsReserve = ();
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
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
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeEvent = RuntimeEvent;
        type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        // Anyone can execute XCM messages locally...
        type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
        type XcmExecuteFilter = Nothing;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Everything;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl ump::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
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
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type XcmExecutor: ExecuteXcm<Self::RuntimeCall>;
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
    pub(super) type ReceivedDmp<T: Config> = StorageValue<_, Vec<Xcm<T::RuntimeCall>>, ValueQuery>;

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
            xcm: VersionedXcm<T::RuntimeCall>,
            max_weight: Weight,
        ) -> Result<Weight, XcmError> {
            let hash = Encode::using_encoded(&xcm, T::Hashing::hash);
            let (result, event) = match Xcm::<T::RuntimeCall>::try_from(xcm) {
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
                    if let Ok(xcm) =
                        VersionedXcm::<T::RuntimeCall>::decode(&mut remaining_fragments)
                    {
                        let _ = Self::handle_xcmp_message(sender, sent_at, xcm, max_weight)
                            .map_err(|e| {
                                debug_assert!(
                                    false,
                                    "Handling XCMP message returned error {:?}",
                                    e
                                );
                            });
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
                let maybe_msg = VersionedXcm::<T::RuntimeCall>::decode(&mut &data[..])
                    .map(Xcm::<T::RuntimeCall>::try_from);
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
