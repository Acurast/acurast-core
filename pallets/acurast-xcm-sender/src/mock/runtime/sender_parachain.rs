use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_support::{
    construct_runtime, parameter_types,
    sp_runtime::{
        testing::Header,
        traits::{AccountIdLookup, Hash},
        AccountId32,
    },
    traits::{Everything, Nothing},
    weights::Weight,
};
pub use pallet_acurast_receiver;
use pallet_xcm::XcmPassthrough;
use polkadot_core_primitives::BlockNumber as RelayBlockNumber;
use polkadot_parachain::primitives::{
    DmpMessageHandler, Id as ParaId, Sibling, XcmpMessageFormat, XcmpMessageHandler,
};
use sp_core::H256;
use sp_std::prelude::*;
use xcm::{latest::prelude::*, VersionedXcm};
use xcm_builder::{
    AccountId32Aliases, AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedRateOfFungible,
    FixedWeightBounds, LocationInverter, NativeAsset, ParentIsPreset, SiblingParachainConvertsVia,
    SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation,
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
    type Lookup = AccountIdLookup<AccountId, ()>;
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

pub type XcmOriginToCallOrigin = (
    SovereignSignedViaLocation<LocationToAccountId, Origin>,
    SignedAccountId32AsNative<RelayNetwork, Origin>,
    XcmPassthrough<Origin>,
);

parameter_types! {
    pub const UnitWeightCost: u64 = 1;
    pub KsmPerSecond: (AssetId, u128) = (Concrete(Parent.into()), 1);
    pub const MaxInstructions: u32 = 100;
}

pub type XcmRouter = crate::tests::ParachainXcmRouter<MsgQueue>;
pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

pub struct XcmConfig;

impl Config for XcmConfig {
    type Call = Call;
    type XcmSender = XcmRouter;
    type AssetTransactor = ();
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

parameter_types! {
    pub const AcurastParachainId: u32 = 2000;
    pub const AcurastReceiverPalletId: u8 = 130;
}

impl crate::Config for Runtime {
    type Event = Event;
    type XcmSender = XcmRouter;
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
        MsgQueue: mock_msg_queue::{Pallet, Storage, Event<T>},
        PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
        AcurastSender: crate::{Pallet, Event<T>},
    }
);
