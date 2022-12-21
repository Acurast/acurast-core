use crate as pallet_acurast_xcm_receiver;
use frame_support::{
    traits::{ConstU16, ConstU64, Everything, Nothing},
    dispatch::{Pays, PostDispatchInfo},
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};
use sp_std::prelude::*;
use xcm::v2::Junction::Parachain;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        AcurastReceiver: pallet_acurast_xcm_receiver::{Pallet, Call, Storage, Event<T>},
        PolkadotXcm: pallet_xcm::{Pallet, Storage, Call, Event<T>, Origin, Config},
    }
);

frame_support::parameter_types! {
    /// The amount of weight an XCM operation takes. This is safe overestimate.
    pub UnitWeightCost: xcm::v2::Weight = 200_000_000;
    /// Maximum number of instructions in a single XCM fragment. A sanity check against
    /// weight caculations getting too crazy.
    pub MaxInstructions: u32 = 100;
    // The ancestry, defines the multilocation describing this consensus system
    pub Ancestry: xcm::v2::MultiLocation = Parachain(2001).into();
}

impl pallet_xcm::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, ()>;
    type XcmRouter = ();
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, ()>;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = ();
    type XcmTeleportFilter = Nothing;
    type XcmReserveTransferFilter = Everything;
    type Weigher = xcm_builder::FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
    type LocationInverter = xcm_builder::LocationInverter<Ancestry>;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub struct OnAcurastFulfillment;
impl crate::traits::OnFulfillment<Test> for OnAcurastFulfillment {
    fn fulfill(
        _payload: Vec<u8>,
        _parameters: Option<Vec<u8>>,
    ) -> frame_support::sp_runtime::DispatchResultWithInfo<PostDispatchInfo> {
        Ok(PostDispatchInfo {
            actual_weight: None,
            pays_fee: Pays::No,
        })
    }
}

pub struct ParachainBarrier;
impl crate::traits::ParachainBarrier<Test> for ParachainBarrier {
    fn ensure_xcm_origin(
        origin: frame_system::pallet_prelude::OriginFor<Test>,
    ) -> Result<(), sp_runtime::DispatchError> {
        // List of allowd parachains
        let allowed_parachains = [
            // Acurast
            xcm::opaque::latest::Junction::Parachain(2001),
        ];

        // Ensure that the call comes from an xcm message
        let location = pallet_xcm::ensure_xcm(origin)?;

        let is_valid_origin = location
            .interior()
            .iter()
            .any(|junction| allowed_parachains.contains(junction));

        if !is_valid_origin {
            return Err(sp_runtime::DispatchError::Other(
                "MultiLocation not allowed.",
            ));
        }

        Ok(())
    }
}

impl pallet_acurast_xcm_receiver::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Payload = Vec<u8>;
    type Parameters = Vec<u8>;
    type OnFulfillment = OnAcurastFulfillment;
    type Barrier = ParachainBarrier;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}
