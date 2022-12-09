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
    let ad = advertisement(1000, 5, None);
    let registration = job_registration_with_reward(script(), 5, 5000, None);
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
        assert_eq!(
            Some(4),
            AcurastMarketplace::stored_capacity(processor_account_id())
        );

        // updating job registration is prohibited after match found
        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration.clone(),
            ),
            Error::<Test>::JobRegistrationUnmodifiable
        );

        let fulfillment = Fulfillment {
            script: registration.script.clone(),
            payload: hex!("00").to_vec(),
        };
        assert_ok!(Acurast::fulfill(
            Origin::signed(processor_account_id()).into(),
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
                Event::Acurast(pallet_acurast::Event::ReceivedFulfillment(
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
    let ad = advertisement(1000, 1, None);
    let registration = job_registration_with_reward(script(), 2, 2000, None);
    let registration2 = job_registration_with_reward(script_random_value(), 2, 2000, None);
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

#[test]
fn test_no_match_insufficient_reputation() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 5, None);
    let registration = job_registration_with_reward(script(), 2, 2000, Some(1));
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
            events(),
            [
                Event::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                // no match event
            ]
        );
    });
}

#[test]
fn test_reputation_update_for_1000_consecutive_fulfills() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1000, None);

    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AcurastMarketplace::advertise(
            Origin::signed(bob_account_id()).into(),
            ad.clone(),
        ));
        for _i in 0..1000 {
            let registration = job_registration_with_reward(random_script(), 5, 5000, None);

            assert_ok!(Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration.clone(),
            ));

            let fulfillment = fulfillment_for(&registration);

            assert_ok!(Acurast::fulfill(
                Origin::signed(bob_account_id()),
                fulfillment.clone(),
                MultiAddress::Id(alice_account_id())
            ));
        }

        assert_eq!(
            Some(crate::BetaParams {
                r: 24_999_951,
                s: 0
            }),
            AcurastMarketplace::stored_reputation(bob_account_id())
        );

        let asset_id = 0;

        assert_eq!(
            Some(1000),
            AcurastMarketplace::total_jobs_assigned(asset_id)
        );
        assert_eq!(Some(5000), AcurastMarketplace::avg_job_reward(asset_id));
    });
}

#[test]
fn test_match_sufficient_reputation() {
    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 5, None);
    let ad1 = advertisement(1000, 5, None);
    let registration1 = job_registration_with_reward(script(), 5, 5000, None);
    let registration2 = job_registration_with_reward(script_random_value(), 5, 5000, Some(1));
    let fulfillment = fulfillment_for(&registration1);

    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(AcurastMarketplace::advertise(
            Origin::signed(bob_account_id()).into(),
            ad.clone(),
        ));

        assert_ok!(Acurast::register(
            Origin::signed(charlie_account_id()).into(),
            registration1.clone(),
        ));

        assert_ok!(Acurast::fulfill(
            Origin::signed(bob_account_id()),
            fulfillment.clone(),
            MultiAddress::Id(charlie_account_id())
        ));

        assert_ok!(AcurastMarketplace::advertise(
            Origin::signed(alice_account_id()).into(),
            ad1.clone(),
        ));

        assert_ok!(Acurast::register(
            Origin::signed(dave_account_id()).into(),
            registration2.clone(),
        ));

        assert_eq!(
            events(),
            [
                Event::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    bob_account_id()
                )),
                // first job assigned to Bob
                Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    charlie_account_id(),
                    registration1.script.clone()
                ))),
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    charlie_account_id()
                )),
                Event::Acurast(pallet_acurast::Event::ReceivedFulfillment(
                    bob_account_id(),
                    fulfillment,
                    registration1,
                    charlie_account_id()
                )),
                Event::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad1.clone(),
                    alice_account_id()
                )),
                Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
                    dave_account_id(),
                    registration2.script.clone()
                ))),
                Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    dave_account_id()
                )),
            ]
        );
    });
}

#[test]
fn test_fulfill_100_processors() {
    ExtBuilder::default().build().execute_with(|| {
        for i in 0..100 {
            let ad = advertisement(1000, 1000, Some(vec![account_id(255 - i)]));
            assert_ok!(AcurastMarketplace::advertise(
                Origin::signed(account_id(i)).into(),
                ad.clone(),
            ));
        }

        for _i in 0..10 {
            for j in 0..100 {
                let registration = job_registration_with_reward(random_script(), 5, 5000, None);

                assert_ok!(Acurast::register(
                    Origin::signed(account_id(255 - j)).into(),
                    registration.clone(),
                ));

                let fulfillment = fulfillment_for(&registration);

                assert_ok!(Acurast::fulfill(
                    Origin::signed(account_id(j)),
                    fulfillment.clone(),
                    MultiAddress::Id(account_id(255 - j))
                ));
            }
        }
        let asset_id = 0;

        assert_eq!(
            Some(1000),
            AcurastMarketplace::total_jobs_assigned(asset_id)
        );
        assert_eq!(Some(5000), AcurastMarketplace::avg_job_reward(asset_id));
    });
}
