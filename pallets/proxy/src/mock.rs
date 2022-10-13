pub mod acurast_runtime {
    use codec::{Decode, Encode};
    use frame_support::{
        construct_runtime, parameter_types,
        sp_runtime::{
            testing::Header,
            traits::{AccountIdLookup, Hash},
            AccountId32,
        },
        traits::{Everything, Nothing},
        weights::{constants::WEIGHT_PER_SECOND, Weight},
        PalletId,
    };
    pub use pallet_acurast;
    use pallet_acurast::LockAndPayAsset;
    use sp_core::H256;
    use sp_std::prelude::*;
    use std::marker::PhantomData;

    use pallet_xcm::XcmPassthrough;
    use polkadot_core_primitives::BlockNumber as RelayBlockNumber;
    use polkadot_parachain::primitives::{
        DmpMessageHandler, Id as ParaId, Sibling, XcmpMessageFormat, XcmpMessageHandler,
    };
    use xcm::{latest::prelude::*, VersionedXcm};
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
        NativeAsset, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};
    pub type AccountId = AccountId32;
    pub type Balance = u128;

    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

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
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type BlockWeights = ();
        type BlockLength = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type DbWeight = ();
        type BaseCallFilter = Everything;
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    parameter_types! {
        pub ExistentialDeposit: Balance = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }

    impl pallet_balances::Config for Runtime {
        type MaxLocks = MaxLocks;
        type Balance = Balance;
        type Event = Event;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    parameter_types! {
        pub const ReservedXcmpWeight: Weight = WEIGHT_PER_SECOND / 4;
        pub const ReservedDmpWeight: Weight = WEIGHT_PER_SECOND / 4;
    }

    parameter_types! {
        pub const KsmLocation: MultiLocation = MultiLocation::parent();
        pub const RelayNetwork: NetworkId = NetworkId::Kusama;
        pub Ancestry: MultiLocation = Parachain(MsgQueue::parachain_id().into()).into();
    }

    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );

    use frame_support::traits::{Get, OriginTrait};
    use xcm_executor::traits::ConvertOrigin;

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

    pub type XcmOriginToCallOrigin = (
        SovereignSignedViaLocation<LocationToAccountId, Origin>,
        SignedAccountId32AsNative<RelayNetwork, Origin>,
        // TODO: safety check of signature
        SignedAccountId32FromXcm<Origin>,
        XcmPassthrough<Origin>,
    );

    parameter_types! {
        pub const UnitWeightCost: Weight = 1;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(Parent.into()), 1);
        pub const MaxInstructions: u32 = 100;
    }

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

    #[frame_support::pallet]
    pub mod mock_msg_queue {
        use super::*;
        use frame_support::pallet_prelude::*;

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
                        match T::XcmExecutor::execute_xcm(location, xcm, max_weight) {
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
            fn handle_xcmp_messages<
                'a,
                I: Iterator<Item = (ParaId, RelayBlockNumber, &'a [u8])>,
            >(
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
                    let maybe_msg = VersionedXcm::<T::Call>::decode(&mut &data[..])
                        .map(Xcm::<T::Call>::try_from);
                    match maybe_msg {
                        Err(_) => {
                            Self::deposit_event(Event::InvalidFormat(id));
                        }
                        Ok(Err(())) => {
                            Self::deposit_event(Event::UnsupportedVersion(id));
                        }
                        Ok(Ok(x)) => {
                            let outcome = T::XcmExecutor::execute_xcm(Parent, x.clone(), limit);
                            <ReceivedDmp<T>>::append(x);
                            Self::deposit_event(Event::ExecutedDownward(id, outcome));
                        }
                    }
                }
                limit
            }
        }
    }

    impl mock_msg_queue::Config for Runtime {
        type Event = Event;
        type XcmExecutor = XcmExecutor<XcmConfig>;
    }

    pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

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

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    type Block = frame_system::mocking::MockBlock<Runtime>;

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            MsgQueue: mock_msg_queue::{Pallet, Storage, Event<T>},
            PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
            Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
            Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>} = 40,
            Assets: pallet_assets,
        }
    );

    parameter_types! {
        pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
        pub const IsRelay: bool = false;
        pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    }

    impl pallet_timestamp::Config for Runtime {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = MinimumPeriod;
        type WeightInfo = ();
    }

    pub const UNIT: Balance = 1_000_000;
    pub const MICROUNIT: Balance = 1;

    impl pallet_assets::Config for Runtime {
        type Event = Event;
        type Balance = Balance;
        type AssetId = u32;
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

    impl pallet_acurast::Config for Runtime {
        type Event = Event;
        type RegistrationExtra = ();
        type FulfillmentRouter = FulfillmentRouter;
        type MaxAllowedSources = frame_support::traits::ConstU16<1000>;
        type AssetTransactor = Transactor;
        type PalletId = AcurastPalletId;
        type RevocationListUpdateBarrier = ();
        type JobAssignmentUpdateBarrier = ();
    }

    pub struct FulfillmentRouter;

    impl pallet_acurast::FulfillmentRouter<Runtime> for FulfillmentRouter {
        fn received_fulfillment(
            _origin: frame_system::pallet_prelude::OriginFor<Runtime>,
            _from: <Runtime as frame_system::Config>::AccountId,
            _fulfillment: pallet_acurast::Fulfillment,
            _registration: pallet_acurast::JobRegistration<
                <Runtime as frame_system::Config>::AccountId,
                <Runtime as pallet_acurast::Config>::RegistrationExtra,
            >,
            _requester: <<Runtime as frame_system::Config>::Lookup as frame_support::sp_runtime::traits::StaticLookup>::Target,
        ) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
            Ok(().into())
        }
    }

    pub struct Transactor;

    impl LockAndPayAsset<Runtime> for Transactor {
        fn lock_asset(
            _asset: MultiAsset,
            _owner: <<Runtime as frame_system::Config>::Lookup as frame_support::sp_runtime::traits::StaticLookup>::Source,
        ) -> Result<(), ()> {
            Ok(())
        }

        fn pay_asset(
            _asset: MultiAsset,
            _target: <<Runtime as frame_system::Config>::Lookup as frame_support::sp_runtime::traits::StaticLookup>::Source,
        ) -> Result<(), ()> {
            Ok(())
        }
    }
}

pub mod proxy_runtime {
    use codec::{Decode, Encode};
    use frame_support::{
        construct_runtime, parameter_types,
        traits::{Everything, Nothing},
        weights::{constants::WEIGHT_PER_SECOND, Weight},
    };
    use sp_core::H256;
    use sp_runtime::{
        testing::Header,
        traits::{AccountIdLookup, Hash},
        AccountId32,
    };
    use sp_std::prelude::*;
    use std::marker::PhantomData;

    use pallet_xcm::XcmPassthrough;
    use polkadot_core_primitives::BlockNumber as RelayBlockNumber;
    use polkadot_parachain::primitives::{
        DmpMessageHandler, Id as ParaId, Sibling, XcmpMessageFormat, XcmpMessageHandler,
    };
    use xcm::{latest::prelude::*, VersionedXcm};
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
        NativeAsset, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};
    pub type AccountId = AccountId32;
    pub type Balance = u128;

    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

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
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type BlockWeights = ();
        type BlockLength = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type DbWeight = ();
        type BaseCallFilter = Everything;
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    parameter_types! {
        pub ExistentialDeposit: Balance = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }

    impl pallet_balances::Config for Runtime {
        type MaxLocks = MaxLocks;
        type Balance = Balance;
        type Event = Event;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    parameter_types! {
        pub const ReservedXcmpWeight: Weight = WEIGHT_PER_SECOND / 4;
        pub const ReservedDmpWeight: Weight = WEIGHT_PER_SECOND / 4;
    }

    parameter_types! {
        pub const KsmLocation: MultiLocation = MultiLocation::parent();
        pub const RelayNetwork: NetworkId = NetworkId::Kusama;
        pub Ancestry: MultiLocation = Parachain(MsgQueue::parachain_id().into()).into();
    }

    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );

    use frame_support::traits::{Get, OriginTrait};
    use xcm_executor::traits::ConvertOrigin;

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

    pub type XcmOriginToCallOrigin = (
        SovereignSignedViaLocation<LocationToAccountId, Origin>,
        SignedAccountId32AsNative<RelayNetwork, Origin>,
        // TODO: safety check of signature
        SignedAccountId32FromXcm<Origin>,
        XcmPassthrough<Origin>,
    );

    parameter_types! {
        pub const UnitWeightCost: Weight = 1;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(Parent.into()), 1);
        pub const MaxInstructions: u32 = 100;
    }

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

    #[frame_support::pallet]
    pub mod mock_msg_queue {
        use super::*;
        use frame_support::pallet_prelude::*;

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
                        match T::XcmExecutor::execute_xcm(location, xcm, max_weight) {
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
            fn handle_xcmp_messages<
                'a,
                I: Iterator<Item = (ParaId, RelayBlockNumber, &'a [u8])>,
            >(
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
                    let maybe_msg = VersionedXcm::<T::Call>::decode(&mut &data[..])
                        .map(Xcm::<T::Call>::try_from);
                    match maybe_msg {
                        Err(_) => {
                            Self::deposit_event(Event::InvalidFormat(id));
                        }
                        Ok(Err(())) => {
                            Self::deposit_event(Event::UnsupportedVersion(id));
                        }
                        Ok(Ok(x)) => {
                            let outcome = T::XcmExecutor::execute_xcm(Parent, x.clone(), limit);
                            <ReceivedDmp<T>>::append(x);
                            Self::deposit_event(Event::ExecutedDownward(id, outcome));
                        }
                    }
                }
                limit
            }
        }
    }

    impl mock_msg_queue::Config for Runtime {
        type Event = Event;
        type XcmExecutor = XcmExecutor<XcmConfig>;
    }

    pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

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

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    type Block = frame_system::mocking::MockBlock<Runtime>;

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            MsgQueue: mock_msg_queue::{Pallet, Storage, Event<T>},
            PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
            AcurastProxy: crate::{Pallet, Call, Event<T>} = 34,
        }
    );

    parameter_types! {
        pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
        pub const IsRelay: bool = false;
        pub Admins: Vec<AccountId> = vec![];
    }

    impl pallet_timestamp::Config for Runtime {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = MinimumPeriod;
        type WeightInfo = ();
    }

    parameter_types! {
    pub const AcurastParachainId: u32 = 2000;
    pub const AcurastPalletId: u8 = 40;
    }

    impl crate::Config for Runtime {
        type Event = Event;
        type AcurastParachainId = AcurastParachainId;
        type AcurastPalletId = AcurastPalletId;
        type XcmSender = XcmRouter;
        type RegistrationExtra = ();
    }
}

pub mod relay_chain {
    use frame_support::{
        construct_runtime, parameter_types,
        sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32},
        traits::{Everything, Nothing},
        weights::Weight,
    };
    use sp_core::H256;

    use polkadot_parachain::primitives::Id as ParaId;
    use polkadot_runtime_parachains::{configuration, origin, shared, ump};
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, ChildParachainAsNative,
        ChildParachainConvertsVia, ChildSystemParachainAsSuperuser,
        CurrencyAdapter as XcmCurrencyAdapter, FixedRateOfFungible, FixedWeightBounds, IsConcrete,
        LocationInverter, SignedAccountId32AsNative, SignedToAccountId32,
        SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};

    pub type AccountId = AccountId32;
    pub type Balance = u128;

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
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type DbWeight = ();
        type BaseCallFilter = Everything;
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    parameter_types! {
        pub ExistentialDeposit: Balance = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }

    impl pallet_balances::Config for Runtime {
        type MaxLocks = MaxLocks;
        type Balance = Balance;
        type Event = Event;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
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
        pub UnitWeightCost: Weight = 1_000;
    }

    pub type SovereignAccountOf = (
        ChildParachainConvertsVia<ParaId, AccountId>,
        AccountId32Aliases<KusamaNetwork, AccountId>,
    );

    pub type LocalAssetTransactor =
        XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, SovereignAccountOf, AccountId, ()>;

    type LocalOriginConverter = (
        SovereignSignedViaLocation<SovereignAccountOf, Origin>,
        ChildParachainAsNative<origin::Origin, Origin>,
        SignedAccountId32AsNative<KusamaNetwork, Origin>,
        ChildSystemParachainAsSuperuser<ParaId, Origin>,
    );

    parameter_types! {
        pub const BaseXcmWeight: Weight = 1_000;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(KsmLocation::get()), 1);
        pub const MaxInstructions: u32 = 100;
    }

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

    impl origin::Config for Runtime {}

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    type Block = frame_system::mocking::MockBlock<Runtime>;

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
}
