use std::marker::PhantomData;

use frame_support::traits::OriginTrait;
use xcm::latest::{Junction, MultiLocation, OriginKind};
use xcm::prelude::*;
use xcm_executor::traits::ConvertOrigin;

use acurast_common::Schedule;
use acurast_runtime::AccountId as AcurastAccountId;
use pallet_acurast::{JobModules, JobRegistration, CU32};
use pallet_acurast_marketplace::{Advertisement, JobRequirements, Pricing, SchedulingWindow};

#[cfg(feature = "runtime-benchmarks")]
pub const SEED: u32 = 1337;

pub type Balance = u128;

pub const SCRIPT_BYTES: [u8; 53] = hex_literal::hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub type MaxAllowedSources = CU32<10>;
pub type MaxSlots = CU32<64>;

pub fn alice_account_id() -> AcurastAccountId {
    [0; 32].into()
}
pub fn bob_account_id() -> AcurastAccountId {
    [1; 32].into()
}
pub fn registration() -> JobRegistration<
    AcurastAccountId,
    MaxAllowedSources,
    JobRequirements<Balance, AcurastAccountId, MaxSlots>,
> {
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
            reward: 20000,
            min_reputation: None,
            instant_match: None,
        },
    }
}
pub fn advertisement(
    fee_per_millisecond: u128,
) -> Advertisement<AcurastAccountId, Balance, CU32<10>> {
    Advertisement {
        pricing: Pricing {
            fee_per_millisecond,
            fee_per_storage_byte: 0,
            base_fee_per_execution: 0,
            scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
        },
        allowed_consumers: None,
        storage_capacity: 5,
        max_memory: 5000,
        network_request_quota: 8,
        available_modules: JobModules::default(),
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
    use sp_runtime::DispatchError;
    use sp_std::prelude::*;
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, NativeAsset,
        ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::XcmExecutor;

    pub use pallet_acurast::{self, CU32};
    pub use pallet_acurast_marketplace;
    use pallet_acurast_marketplace::{AssetRewardManager, JobRequirements};

    use super::Balance;

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

    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>} = 0,
            Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
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
        pub ExistentialDeposit: Balance = 1;
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
        pub KsmPerSecond: (AssetId, u128, u128) = (Concrete(Parent.into()), 1, 0);
        pub const MaxInstructions: u32 = 100;
        pub UniversalLocation: InteriorMultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
        pub const MaxAssetsIntoHolding: u32 = 64;
        pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
    }

    pub struct XcmConfig;

    impl xcm_executor::Config for XcmConfig {
        type RuntimeCall = RuntimeCall;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = XcmOriginToCallOrigin;
        type IsReserve = NativeAsset;
        type IsTeleporter = ();
        type UniversalLocation = UniversalLocation;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
        type AssetLocker = ();
        type AssetExchanger = ();
        type PalletInstancesInfo = AllPalletsWithSystem;
        type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
        type FeeManager = ();
        type MessageExporter = ();
        type UniversalAliases = Nothing;
        type CallDispatcher = RuntimeCall;
        type SafeCallFilter = Everything;
    }

    impl pallet_balances::Config for Runtime {
        type Balance = Balance;
        type DustRemoval = ();
        type RuntimeEvent = RuntimeEvent;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
        type HoldIdentifier = [u8; 8];
        type FreezeIdentifier = ();
        // Holds are used with COLLATOR_LOCK_ID and DELEGATOR_LOCK_ID
        type MaxHolds = ConstU32<2>;
        type MaxFreezes = ConstU32<0>;
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
        type AccountData = pallet_balances::AccountData<Balance>;
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

    #[cfg(feature = "runtime-benchmarks")]
    pub struct TestBenchmarkHelper;

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
        type RegistrationExtra = JobRequirements<Balance, AccountId, super::MaxSlots>;
        type MaxAllowedSources = super::MaxAllowedSources;
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
    impl pallet_acurast::BenchmarkHelper<Runtime> for TestBenchmarkHelper {
        fn registration_extra() -> <Runtime as pallet_acurast::Config>::RegistrationExtra {
            JobRequirements {
                slots: 1,
                reward: 1,
                min_reputation: None,
                instant_match: None,
            }
        }

        fn funded_account(index: u32) -> super::AcurastAccountId {
            let caller: super::AcurastAccountId =
                frame_benchmarking::account("token_account", index, super::SEED);
            <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(
                &caller,
                u32::MAX.into(),
            );

            caller
        }
    }

    pub struct ManagerOf;

    impl pallet_acurast_marketplace::ManagerProvider<Runtime> for ManagerOf {
        fn manager_of(
            owner: &<Runtime as frame_system::Config>::AccountId,
        ) -> Result<<Runtime as frame_system::Config>::AccountId, DispatchError> {
            Ok(owner.clone())
        }
    }

    pub struct ProcessorLastSeenProvider;

    impl pallet_acurast_marketplace::traits::ProcessorLastSeenProvider<Runtime>
        for ProcessorLastSeenProvider
    {
        fn last_seen(_processor: &<Runtime as frame_system::Config>::AccountId) -> Option<u128> {
            Some(AcurastMarketplace::now().unwrap().into())
        }
    }

    impl pallet_acurast_marketplace::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type MaxAllowedConsumers = CU32<4>;
        type MaxProposedMatches = frame_support::traits::ConstU32<10>;
        type MaxSlots = CU32<64>;
        type MaxFinalizeJobs = frame_support::traits::ConstU32<10>;
        type RegistrationExtra = JobRequirements<Balance, AccountId, Self::MaxSlots>;
        type PalletId = AcurastPalletId;
        type ReportTolerance = ReportTolerance;
        type Balance = Balance;
        type ManagerProvider = ManagerOf;
        type RewardManager = AssetRewardManager<FeeManagerImpl, Balances, AcurastMarketplace>;
        type ProcessorLastSeenProvider = ProcessorLastSeenProvider;
        type MarketplaceHooks = ();
        type WeightInfo = pallet_acurast_marketplace::weights::WeightInfo<Runtime>;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = TestBenchmarkHelper;
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl pallet_acurast_marketplace::BenchmarkHelper<Runtime> for TestBenchmarkHelper {
        fn registration_extra(
            r: pallet_acurast_marketplace::JobRequirementsFor<Runtime>,
        ) -> <Runtime as pallet_acurast_marketplace::Config>::RegistrationExtra {
            r
        }

        fn funded_account(index: u32, amount: Balance) -> super::AcurastAccountId {
            let caller: super::AcurastAccountId =
                frame_benchmarking::account("token_account", index, super::SEED);
            <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(&caller, amount);

            caller
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
        type UniversalLocation = UniversalLocation;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
        type Currency = Balances;
        type CurrencyMatcher = ();
        type TrustedLockers = ();
        type SovereignAccountOf = LocationToAccountId;
        type MaxLockers = ConstU32<8>;
        type WeightInfo = pallet_xcm::TestWeightInfo;
        type AdminOrigin = EnsureRoot<AccountId>;
        type MaxRemoteLockConsumers = ConstU32<0>;
        type RemoteLockConsumerIdentifier = ();
        #[cfg(feature = "runtime-benchmarks")]
        type ReachableDest = ReachableDest;
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
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, NativeAsset,
        ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};

    use pallet_acurast::CU32;
    use pallet_acurast_marketplace::JobRequirements;

    use crate::mock::Balance;

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
        type UniversalLocation = UniversalLocation;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
        type AssetLocker = ();
        type AssetExchanger = ();
        type PalletInstancesInfo = AllPalletsWithSystem;
        type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
        type FeeManager = ();
        type MessageExporter = ();
        type UniversalAliases = Nothing;
        type CallDispatcher = RuntimeCall;
        type SafeCallFilter = Everything;
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
            ParachainInfo: parachain_info,
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            MsgQueue: super::mock_msg_queue::{Pallet, Storage, Event<T>},
            PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
            AcurastProxy: crate::{Pallet, Call, Event<T>} = 34,
        }
    );

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub UniversalLocation: InteriorMultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
        pub const MaxAssetsIntoHolding: u32 = 64;
    }
    parameter_types! {
        pub ExistentialDeposit: Balance = 1;
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
        pub KsmPerSecond: (AssetId, u128, u128) = (Concrete(Parent.into()), 1, 0);
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
        pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
    }

    impl parachain_info::Config for Runtime {}

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
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    impl pallet_balances::Config for Runtime {
        type Balance = Balance;
        type DustRemoval = ();
        type RuntimeEvent = RuntimeEvent;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
        type HoldIdentifier = [u8; 8];
        type FreezeIdentifier = ();
        // Holds are used with COLLATOR_LOCK_ID and DELEGATOR_LOCK_ID
        type MaxHolds = ConstU32<2>;
        type MaxFreezes = ConstU32<0>;
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
        type UniversalLocation = UniversalLocation;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
        type Currency = Balances;
        type CurrencyMatcher = ();
        type TrustedLockers = ();
        type SovereignAccountOf = LocationToAccountId;
        type MaxLockers = ConstU32<8>;
        type WeightInfo = pallet_xcm::TestWeightInfo;
        type AdminOrigin = EnsureRoot<AccountId>;
        type MaxRemoteLockConsumers = ConstU32<0>;
        type RemoteLockConsumerIdentifier = ();
        #[cfg(feature = "runtime-benchmarks")]
        type ReachableDest = ReachableDest;
    }

    impl crate::Config for Runtime {
        type RuntimeEvent = RuntimeEvent;
        type RegistrationExtra = JobRequirements<Balance, AccountId, super::MaxSlots>;
        type MaxAllowedSources = super::MaxAllowedSources;
        type MaxAllowedConsumers = CU32<10>;
        type Balance = Balance;
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
            <Runtime as crate::Config>::MaxAllowedSources,
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
            <Runtime as crate::Config>::Balance,
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
    use polkadot_parachain::primitives::{Id as ParaId, Sibling};
    use polkadot_runtime_parachains::{configuration, origin, shared, ump};
    use sp_core::{ConstU32, H256};
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, ChildParachainAsNative,
        ChildParachainConvertsVia, ChildSystemParachainAsSuperuser,
        CurrencyAdapter as XcmCurrencyAdapter, FixedRateOfFungible, FixedWeightBounds, IsConcrete,
        ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};

    use crate::mock::Balance;

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
        type UniversalLocation = UniversalLocation;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
        type AssetLocker = ();
        type AssetExchanger = ();
        type PalletInstancesInfo = AllPalletsWithSystem;
        type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
        type FeeManager = ();
        type MessageExporter = ();
        type UniversalAliases = Nothing;
        type CallDispatcher = RuntimeCall;
        type SafeCallFilter = Everything;
    }

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
            ParachainInfo: parachain_info,
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            ParasOrigin: origin::{Pallet, Origin},
            ParasUmp: ump::{Pallet, Call, Storage, Event},
            XcmPallet: pallet_xcm::{Pallet, Call, Storage, Event<T>, Origin},
        }
    );

    parameter_types! {
        pub const KsmLocation: MultiLocation = MultiLocation { parents: 0, interior: Here };
        pub const KusamaNetwork: NetworkId = NetworkId::Kusama;
        pub Ancestry: MultiLocation = Here.into();
        pub UnitWeightCost: u64 = 1_000;
        pub UniversalLocation: InteriorMultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
        pub const MaxAssetsIntoHolding: u32 = 64;
        pub const RelayNetwork: NetworkId = NetworkId::Kusama;
    }
    parameter_types! {
        pub const BaseXcmWeight: u64 = 1_000;
        pub KsmPerSecond: (AssetId, u128, u128) = (Concrete(KsmLocation::get()), 1, 0);
        pub const MaxInstructions: u32 = 100;
    }
    parameter_types! {
        pub const FirstMessageFactorPercent: u64 = 100;
    }
    parameter_types! {
        pub ExistentialDeposit: Balance = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
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
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    impl parachain_info::Config for Runtime {}

    impl pallet_balances::Config for Runtime {
        type Balance = Balance;
        type DustRemoval = ();
        type RuntimeEvent = RuntimeEvent;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
        type HoldIdentifier = [u8; 8];
        type FreezeIdentifier = ();
        // Holds are used with COLLATOR_LOCK_ID and DELEGATOR_LOCK_ID
        type MaxHolds = ConstU32<2>;
        type MaxFreezes = ConstU32<0>;
    }

    impl shared::Config for Runtime {}

    impl configuration::Config for Runtime {
        type WeightInfo = configuration::TestWeightInfo;
    }

    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );

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
        type UniversalLocation = UniversalLocation;
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
        type Currency = Balances;
        type CurrencyMatcher = ();
        type TrustedLockers = ();
        type SovereignAccountOf = LocationToAccountId;
        type MaxLockers = ConstU32<8>;
        type WeightInfo = pallet_xcm::TestWeightInfo;
        type AdminOrigin = EnsureRoot<AccountId>;
        type MaxRemoteLockConsumers = ConstU32<0>;
        type RemoteLockConsumerIdentifier = ();
        #[cfg(feature = "runtime-benchmarks")]
        type ReachableDest = ReachableDest;
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
    use xcm_simulator::{MultiLocation, ParaId, RelayBlockNumber};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type XcmExecutor: ExecuteXcm<Self::RuntimeCall>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::pallet]
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
                    let location = MultiLocation::new(1, Parachain(sender.into()));
                    match T::XcmExecutor::execute_xcm(location, xcm, [0; 32], max_weight) {
                        Outcome::Error(e) => (Err(e.clone()), Event::Fail(Some(hash), e)),
                        Outcome::Complete(w) => (Ok(w), Event::Success(Some(hash))),
                        // As far as the caller is concerned, this was dispatched without error, so
                        // we just report the weight used.
                        Outcome::Incomplete(w, e) => (Ok(w), Event::Fail(Some(hash), e)),
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
                            T::XcmExecutor::execute_xcm(Parent, x.clone(), [0; 32], limit);
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
