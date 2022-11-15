#![cfg(test)]

use frame_support::{assert_err, assert_ok};

use crate::{Error, JobStatus};
use crate::mock::*;

#[test]
fn test_match() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 5);
    let registration = job_registration_with_reward(script(), 5, 5000);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AcurastMarketplace::advertise(
            Origin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(ad.clone()),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Assigned),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );

        // updating job registration is prohibited after match found
        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration.clone(),
            ),
            Error::<Test>::JobRegistrationUnmodifiable
        );

        assert_eq!(
            events(),
            [
                Event::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    alice_account_id(),
                    registration.script.clone()
                ),)),
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
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
            Origin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(ad.clone()),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );

        // the first job matches because 1 capacity left
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_eq!(
            Some(0),
            AcurastMarketplace::stored_capacity(processor_account_id())
        );

        // this one does not match anymore
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration2.clone(),
        ));

        assert_eq!(
            events(),
            [
                Event::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                // first job
                Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    alice_account_id(),
                    registration.script.clone()
                ),)),
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                // second job
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    alice_account_id()
                )),
                // no match event
            ]
        );
    });
}
