#![cfg(test)]

use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use pallet_acurast::Fulfillment;
use sp_runtime::MultiAddress;

use crate::mock::*;
use crate::stub::*;
use crate::{Error, JobStatus, SLAEvaluation};

#[test]
fn test_match() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 5);
    let registration = job_registration_with_reward(script(), 5, 5000);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(ad.clone()),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Assigned),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );
        assert_eq!(
            Some(4),
            AcurastMarketplace::stored_capacity(processor_account_id())
        );

        // updating job registration is prohibited after match found
        assert_err!(
            Acurast::register(
                RuntimeOrigin::signed(alice_account_id()).into(),
                registration.clone(),
            ),
            Error::<Test>::JobRegistrationUnmodifiable
        );

        let fulfillment = Fulfillment {
            script: registration.script.clone(),
            payload: hex!("00").to_vec(),
        };
        assert_ok!(Acurast::fulfill(
            RuntimeOrigin::signed(processor_account_id()).into(),
            fulfillment.clone(),
            MultiAddress::Id(alice_account_id()),
        ));
        assert_eq!(
            Some(JobStatus::Fulfilled(SLAEvaluation { total: 1, met: 1 })),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );
        assert_eq!(
            Some(5),
            AcurastMarketplace::stored_capacity(processor_account_id())
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    alice_account_id(),
                    registration.script.clone()
                ),)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                RuntimeEvent::Acurast(pallet_acurast::Event::ReceivedFulfillment(
                    processor_account_id(),
                    fulfillment,
                    registration,
                    alice_account_id()
                )),
            ]
        );
    });
}

#[test]
fn test_no_match_insufficient_capacity() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1);
    let registration = job_registration_with_reward(script(), 2, 2000);
    let registration2 = job_registration_with_reward(script_random_value(), 2, 2000);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(ad.clone()),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );

        // the first job matches because 1 capacity left
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_eq!(
            Some(0),
            AcurastMarketplace::stored_capacity(processor_account_id())
        );

        // this one does not match anymore
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration2.clone(),
        ));

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                // first job
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    alice_account_id(),
                    registration.script.clone()
                ),)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                // second job
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    alice_account_id()
                )),
                // no match event
            ]
        );
    });
}
