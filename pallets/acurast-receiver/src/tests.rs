use crate::mock::*;
use frame_support::{assert_err, assert_ok, traits::ConstU32, BoundedVec};
use xcm::v2::{Junction::Parachain, Junctions::X1, MultiLocation};

#[test]
fn test_fulfill() {
	let payload: BoundedVec<u8, ConstU32<128>> = vec![0; 128].try_into().unwrap();

	new_test_ext().execute_with(|| {
		// Mock and xcm origin
		let xcm_origin = Origin::from(pallet_xcm::Origin::Xcm(MultiLocation {
			parents: 1,
			interior: X1(Parachain(2001)),
		}));

		// Dispatch fulfill extrinsic with valid origin.
		assert_ok!(AcurastReceiver::fulfill(xcm_origin, payload.clone()));
	});

	new_test_ext().execute_with(|| {
		// Mock and xcm origin
		let xcm_origin = Origin::from(pallet_xcm::Origin::Xcm(MultiLocation {
			parents: 1,
			interior: X1(Parachain(2000)),
		}));

		// Dispatch fulfill extrinsic with wrong origin.
		assert_err!(
			AcurastReceiver::fulfill(xcm_origin, payload.clone()),
			"MultiLocation not allowed."
		);
	});
}
