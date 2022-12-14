use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, Everything, Nothing},
    weights::Weight,
    BoundedVec,
};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{AccountIdLookup, Hash},
    AccountId32,
};
use sp_std::prelude::*;

use pallet_xcm::XcmPassthrough;
use polkadot_core_primitives::BlockNumber as RelayBlockNumber;
use polkadot_parachain::primitives::{
    DmpMessageHandler, Id as ParaId, XcmpMessageFormat, XcmpMessageHandler,
};
use xcm::{latest::prelude::*, VersionedXcm};
use xcm_builder::{
    AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds,
    LocationInverter, NativeAsset, SignedAccountId32AsNative, SignedToAccountId32,
};
use xcm_executor::{Config, XcmExecutor};
pub type AccountId = AccountId32;

impl frame_system::Config for Runtime {
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
    type BlockHashCount = ();
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

pub type XcmOriginToCallOrigin = (
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    XcmPassthrough<RuntimeOrigin>,
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
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    type AssetTransactor = ();
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

#[frame_support::pallet]
pub mod mock_msg_queue {
    use super::*;
    use frame_support::pallet_prelude::*;

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

impl mock_msg_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

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

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub struct AllowAcurastBarrier {}
impl<T: pallet_acurast_receiver::Config> pallet_acurast_receiver::traits::ParachainBarrier<T>
    for AllowAcurastBarrier
{
    fn ensure_xcm_origin(
        _: <T as frame_system::Config>::RuntimeOrigin,
    ) -> Result<(), sp_runtime::DispatchError> {
        Ok(())
    }
}

impl pallet_acurast_receiver::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Payload = BoundedVec<u8, ConstU32<128>>;
    type Parameters = BoundedVec<u8, ConstU32<128>>;
    type OnFulfillment = OnFulfillment;
    type Barrier = AllowAcurastBarrier;
}

pub struct OnFulfillment;
impl pallet_acurast_receiver::traits::OnFulfillment<Runtime> for OnFulfillment {
    fn fulfill(
        _fulfillment: Vec<u8>,
        _parameters: Option<Vec<u8>>,
    ) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
        Ok(().into())
    }
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
        AcurastReceiver: pallet_acurast_receiver::{Pallet, Call, Storage, Event<T>} = 130
    }
);
