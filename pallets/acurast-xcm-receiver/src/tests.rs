use crate::mock::*;
use frame_support::{assert_err, assert_ok};
use xcm::v2::{Junction::Parachain, Junctions::X1, MultiLocation};

#[test]
fn test_fulfill() {
    let payload = vec![0u8; 128];

    new_test_ext().execute_with(|| {
        // Mock and xcm origin
        let xcm_origin = RuntimeOrigin::from(pallet_xcm::Origin::Xcm(MultiLocation {
            parents: 1,
            interior: X1(Parachain(2001)),
        }));

        // Dispatch fulfill extrinsic with valid origin.
        assert_ok!(AcurastReceiver::fulfill(xcm_origin, payload.clone(), None));
    });

    new_test_ext().execute_with(|| {
        // Mock and xcm origin
        let xcm_origin = RuntimeOrigin::from(pallet_xcm::Origin::Xcm(MultiLocation {
            parents: 1,
            interior: X1(Parachain(2000)),
        }));

        // Dispatch fulfill extrinsic with wrong origin.
        assert_err!(
            AcurastReceiver::fulfill(xcm_origin, payload.clone(), None),
            "MultiLocation not allowed."
        );
    });
}
