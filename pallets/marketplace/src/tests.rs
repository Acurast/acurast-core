#![cfg(test)]

use frame_support::{assert_err, assert_ok, traits::Hooks};

use pallet_acurast::utils::validate_and_extract_attestation;
use pallet_acurast::JobRegistrationFor;
use pallet_acurast::Schedule;

use crate::stub::*;
use crate::{
    mock::*, AdvertisementRestriction, Assignment, Error, ExecutionResult, JobStatus, Match, SLA,
};
use crate::{JobRequirements, PlannedExecution};

#[test]
fn test_match() {
    let now = 1_671_789_600_000; // 23.12.2022 10:00;

    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1, 100_000, 50_000, 8);
    let asset_id = ad.pricing[0].reward_asset;
    let registration = JobRegistrationFor::<Test> {
        script: script(),
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
        extra: JobRequirements {
            slots: 1,
            reward: asset(3_000_000 * 2),
            min_reputation: None,
            instant_match: None,
        },
    };
    let job_id = (alice_account_id(), registration.script.clone());
    let m = Match {
        job_id: job_id.clone(),
        sources: vec![PlannedExecution {
            source: processor_account_id(),
            start_delay: 0,
        }],
    };

    ExtBuilder::default().build().execute_with(|| {
        // pretend current time
        later(now);

        let chain = attestation_chain();
        assert_ok!(Acurast::submit_attestation(
            RuntimeOrigin::signed(processor_account_id()).into(),
            chain.clone()
        ));
        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();

        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(AdvertisementRestriction {
                max_memory: 50_000,
                network_request_quota: 8,
                storage_capacity: 100_000,
                allowed_consumers: ad.allowed_consumers.clone()
            }),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );
        assert_eq!(
            Some(ad.pricing[0].clone()),
            AcurastMarketplace::stored_advertisement_pricing(
                processor_account_id(),
                asset_id.clone()
            )
        );

        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );
        assert_eq!(
            Some(100_000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        assert_ok!(AcurastMarketplace::propose_matching(
            RuntimeOrigin::signed(charlie_account_id()).into(),
            vec![m.clone()],
        ));
        assert_eq!(
            Some(JobStatus::Matched),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );
        assert_eq!(
            Some(80_000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        // updating job registration is prohibited after match found
        assert_err!(
            Acurast::register(
                RuntimeOrigin::signed(alice_account_id()).into(),
                registration.clone(),
            ),
            Error::<Test>::JobRegistrationUnmodifiable
        );

        assert_ok!(AcurastMarketplace::acknowledge_match(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Assigned(1)),
            AcurastMarketplace::stored_job_status(alice_account_id(), script())
        );

        // pretend time moved on
        assert_eq!(1, System::block_number());
        later(registration.schedule.start_time + 3000); // pretend actual execution until report call took 3 seconds
        assert_eq!(2, System::block_number());

        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            false,
            ExecutionResult::Success(operation_hash())
        ));
        assert_eq!(
            Some(Assignment {
                slot: 0,
                start_delay: 0,
                fee_per_execution: MockAsset {
                    id: 0,
                    amount: 5_020_000
                },
                acknowledged: true,
                sla: SLA { total: 2, met: 1 },
            }),
            AcurastMarketplace::stored_matches(processor_account_id(), job_id.clone()),
        );
        // Job still assigned after one execution
        assert_eq!(
            Some(JobStatus::Assigned(1)),
            AcurastMarketplace::stored_job_status(alice_account_id(), script()),
        );
        assert_eq!(
            Some(80000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        // pretend time moved on
        later(registration.schedule.range(0).unwrap().1 - 2000);
        assert_eq!(3, System::block_number());

        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            true,
            ExecutionResult::Success(operation_hash())
        ));
        assert_eq!(
            None,
            AcurastMarketplace::stored_matches(processor_account_id(), job_id.clone()),
        );
        assert_eq!(
            Some(1),
            AcurastMarketplace::total_assigned(asset_id.clone())
        );
        assert_eq!(
            Some(6000000),
            AcurastMarketplace::average_reward(asset_id.clone())
        );
        // Job no longer assigned after last execution
        assert_eq!(
            None,
            AcurastMarketplace::stored_job_status(alice_account_id(), script()),
        );
        assert_eq!(
            Some(100_000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::Acurast(pallet_acurast::Event::AttestationStored(
                    attestation,
                    processor_account_id()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(MockAsset {
                    id: 0,
                    amount: 12_000_000
                })),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(m)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(MockAsset {
                    id: 0,
                    amount: 1_960_000 // this is before splitting of the configured percentage that actually is transfered to the matcher
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssigned(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 0 },
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(MockAsset {
                    id: 0,
                    amount: 5_020_000
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 1 },
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(MockAsset {
                    id: 0,
                    amount: 5_020_000
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 2 },
                    }
                )),
            ]
        );
    });
}

#[test]
fn test_no_match_schedule_overlap() {
    let now = 1_671_789_600_000; // 23.12.2022 10:00;

    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1, 100_000, 50_000, 8);
    let registration1 = JobRegistrationFor::<Test> {
        script: script(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 5000,
            start_time: 1_671_800_400_000, // 23.12.2022 13:00
            end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
            interval: 1_800_000,           // 30min -> 2 executions fit
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        extra: JobRequirements {
            slots: 1,
            reward: asset(3_000_000 * 2),
            min_reputation: None,
            instant_match: None,
        },
    };
    let job_id1 = (alice_account_id(), registration1.script.clone());

    let registration2 = JobRegistrationFor::<Test> {
        script: script_random_value(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 5000,
            start_time: 1_671_802_200_000, // 23.12.2022 13:30
            end_time: 1_671_805_800_000,   // 23.12.2022 14:30 (one hour later)
            interval: 1_200_000,           // 20min -> 3 executions fit
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        extra: JobRequirements {
            slots: 1,
            reward: asset(3_000_000 * 2),
            min_reputation: None,
            instant_match: None,
        },
    };
    let _job_id2 = (alice_account_id(), registration2.script.clone());

    ExtBuilder::default().build().execute_with(|| {
        // pretend current time
        assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));

        // register first job
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration1.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(alice_account_id(), registration1.script.clone())
        );

        // register second job
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration2.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(alice_account_id(), registration2.script.clone())
        );

        // the first job matches because capacity left
        let m = Match {
            job_id: job_id1.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        assert_ok!(AcurastMarketplace::propose_matching(
            RuntimeOrigin::signed(charlie_account_id()).into(),
            vec![m.clone()],
        ));

        // this one does not match anymore
        let m = Match {
            job_id: job_id1.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        assert_err!(
            AcurastMarketplace::propose_matching(
                RuntimeOrigin::signed(charlie_account_id()).into(),
                vec![m.clone()],
            ),
            Error::<Test>::ScheduleOverlapInMatch
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(MockAsset {
                    id: 0,
                    amount: 12_000_000
                })),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    alice_account_id()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(MockAsset {
                    id: 0,
                    amount: 18_000_000
                })),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    alice_account_id()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(m)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(MockAsset {
                    id: 0,
                    amount: 1_960_000 // this is before splitting of the configured percentage that actually is transfered to the matcher
                })),
                // no match event for second
            ]
        );
    });
}

#[test]
fn test_no_match_insufficient_reputation() {
    let now = 1_671_789_600_000; // 23.12.2022 10:00;

    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1, 100_000, 50_000, 8);
    let registration1 = JobRegistrationFor::<Test> {
        script: script(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 5000,
            start_time: 1_671_800_400_000, // 23.12.2022 13:00
            end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
            interval: 1_800_000,           // 30min -> 2 executions fit
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        extra: JobRequirements {
            slots: 1,
            reward: asset(3_000_000 * 2),
            min_reputation: Some(1_000_000),
            instant_match: None,
        },
    };
    let job_id1 = (alice_account_id(), registration1.script.clone());

    ExtBuilder::default().build().execute_with(|| {
        // pretend current time
        assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));

        // register job
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration1.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(alice_account_id(), registration1.script.clone())
        );

        // the job matches except inssufficient reputation
        let m = Match {
            job_id: job_id1.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        assert_err!(
            AcurastMarketplace::propose_matching(
                RuntimeOrigin::signed(charlie_account_id()).into(),
                vec![m.clone()],
            ),
            Error::<Test>::InsufficientReputationInMatch
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(MockAsset {
                    id: 0,
                    amount: 12_000_000
                })),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    alice_account_id()
                )),
                // no match event for job
            ]
        );
    });
}

#[test]
fn test_more_reports_than_expected() {
    let now = 1_671_789_600_000; // 23.12.2022 10:00;

    // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
    let ad = advertisement(1000, 1, 100_000, 50_000, 8);
    let registration = JobRegistrationFor::<Test> {
        script: script(),
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
        extra: JobRequirements {
            slots: 1,
            reward: asset(3_000_000 * 2),
            min_reputation: None,
            instant_match: None,
        },
    };
    let job_id = (alice_account_id(), registration.script.clone());
    ExtBuilder::default().build().execute_with(|| {
        // pretend current time
        assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
        assert_ok!(AcurastMarketplace::advertise(
            RuntimeOrigin::signed(processor_account_id()).into(),
            ad.clone(),
        ));
        assert_eq!(
            Some(AdvertisementRestriction {
                max_memory: 50_000,
                network_request_quota: 8,
                storage_capacity: 100_000,
                allowed_consumers: ad.allowed_consumers.clone()
            }),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );

        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration.clone(),
        ));

        let m = Match {
            job_id: job_id.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        assert_ok!(AcurastMarketplace::propose_matching(
            RuntimeOrigin::signed(charlie_account_id()).into(),
            vec![m.clone()],
        ));

        assert_ok!(AcurastMarketplace::acknowledge_match(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
        ));

        // report twice with success
        // -------------------------

        // pretend time moved on
        let mut iter = registration.schedule.iter(0).unwrap();
        later(iter.next().unwrap() + 1000);
        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            false,
            ExecutionResult::Success(operation_hash())
        ));

        // pretend time moved on
        later(iter.next().unwrap() + 1000);
        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            false,
            ExecutionResult::Success(operation_hash())
        ));

        // third report is illegal!
        later(registration.schedule.range(0).unwrap().1 + 1000);
        assert_err!(
            AcurastMarketplace::report(
                RuntimeOrigin::signed(processor_account_id()).into(),
                job_id.clone(),
                true,
                ExecutionResult::Success(operation_hash())
            ),
            Error::<Test>::MoreReportsThanExpected
        );

        assert_eq!(
            events(),
            [
                RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStored(
                    ad.clone(),
                    processor_account_id()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(MockAsset {
                    id: 0,
                    amount: 12_000_000
                })),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(m)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(MockAsset {
                    id: 0,
                    amount: 1_960_000 // this is before splitting of the configured percentage that actually is transfered to the matcher
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssigned(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 0 },
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(MockAsset {
                    id: 0,
                    amount: 5_020_000
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 1 },
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(MockAsset {
                    id: 0,
                    amount: 5_020_000
                })),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: MockAsset {
                            id: 0,
                            amount: 5_020_000
                        },
                        acknowledged: true,
                        sla: SLA { total: 2, met: 2 },
                    }
                )),
            ]
        );
    });
}

// #[test]
// fn test_reputation_update_for_1000_consecutive_fulfills() {
//     // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
//     let ad = advertisement(1000, 1000, None);
//
//     ExtBuilder::default().build().execute_with(|| {
//         assert_ok!(AcurastMarketplace::advertise(
//             Origin::signed(bob_account_id()).into(),
//             ad.clone(),
//         ));
//         for _i in 0..1000 {
//             let registration = job_registration_with_reward(random_script(), 5, 5000, None);
//
//             assert_ok!(Acurast::register(
//                 Origin::signed(alice_account_id()).into(),
//                 registration.clone(),
//             ));
//
//             let fulfillment = fulfillment_for(&registration);
//
//             assert_ok!(Acurast::fulfill(
//                 Origin::signed(bob_account_id()),
//                 fulfillment.clone(),
//                 MultiAddress::Id(alice_account_id())
//             ));
//         }
//
//         assert_eq!(
//             Some(crate::BetaParams {
//                 r: 24_999_951,
//                 s: 0
//             }),
//             AcurastMarketplace::stored_reputation(bob_account_id())
//         );
//
//         let asset_id = 0;
//
//         assert_eq!(
//             Some(1000),
//             AcurastMarketplace::total_assigned(asset_id)
//         );
//         assert_eq!(Some(5000), AcurastMarketplace::average_reward(asset_id));
//     });
// }
//
// #[test]
// fn test_match_sufficient_reputation() {
//     // 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
//     let ad = advertisement(1000, 5, None);
//     let ad1 = advertisement(1000, 5, None);
//     let registration1 = job_registration_with_reward(script(), 5, 5000, None);
//     let registration2 = job_registration_with_reward(script_random_value(), 5, 5000, Some(1));
//     let fulfillment = fulfillment_for(&registration1);
//
//     ExtBuilder::default().build().execute_with(|| {
//         assert_ok!(AcurastMarketplace::advertise(
//             Origin::signed(bob_account_id()).into(),
//             ad.clone(),
//         ));
//
//         assert_ok!(Acurast::register(
//             Origin::signed(charlie_account_id()).into(),
//             registration1.clone(),
//         ));
//
//         assert_ok!(Acurast::fulfill(
//             Origin::signed(bob_account_id()),
//             fulfillment.clone(),
//             MultiAddress::Id(charlie_account_id())
//         ));
//
//         assert_ok!(AcurastMarketplace::advertise(
//             Origin::signed(alice_account_id()).into(),
//             ad1.clone(),
//         ));
//
//         assert_ok!(Acurast::register(
//             Origin::signed(dave_account_id()).into(),
//             registration2.clone(),
//         ));
//
//         assert_eq!(
//             events(),
//             [
//                 Event::AcurastMarketplace(crate::Event::AdvertisementStored(
//                     ad.clone(),
//                     bob_account_id()
//                 )),
//                 // first job assigned to Bob
//                 Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
//                     charlie_account_id(),
//                     registration1.script.clone()
//                 ))),
//                 Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
//                     registration1.clone(),
//                     charlie_account_id()
//                 )),
//                 Event::Acurast(pallet_acurast::Event::ReceivedFulfillment(
//                     bob_account_id(),
//                     fulfillment,
//                     registration1,
//                     charlie_account_id()
//                 )),
//                 Event::AcurastMarketplace(crate::Event::AdvertisementStored(
//                     ad1.clone(),
//                     alice_account_id()
//                 )),
//                 Event::AcurastMarketplace(crate::Event::JobRegistrationMatched((
//                     dave_account_id(),
//                     registration2.script.clone()
//                 ))),
//                 Event::Acurast(pallet_acurast::Event::JobRegistrationStored(
//                     registration2.clone(),
//                     dave_account_id()
//                 )),
//             ]
//         );
//     });
// }
//
// #[test]
// fn test_fulfill_100_processors() {
//     ExtBuilder::default().build().execute_with(|| {
//         for i in 0..100 {
//             let ad = advertisement(1000, 1000, Some(vec![account_id(255 - i)]));
//             assert_ok!(AcurastMarketplace::advertise(
//                 Origin::signed(account_id(i)).into(),
//                 ad.clone(),
//             ));
//         }
//
//         for _i in 0..10 {
//             for j in 0..100 {
//                 let registration = job_registration_with_reward(random_script(), 5, 5000, None);
//
//                 assert_ok!(Acurast::register(
//                     Origin::signed(account_id(255 - j)).into(),
//                     registration.clone(),
//                 ));
//
//                 let fulfillment = fulfillment_for(&registration);
//
//                 assert_ok!(Acurast::fulfill(
//                     Origin::signed(account_id(j)),
//                     fulfillment.clone(),
//                     MultiAddress::Id(account_id(255 - j))
//                 ));
//             }
//         }
//         let asset_id = 0;
//
//         assert_eq!(
//             Some(1000),
//             AcurastMarketplace::total_assigned(asset_id)
//         );
//         assert_eq!(Some(5000), AcurastMarketplace::average_reward(asset_id));
//     });
// }

fn next_block() {
    if System::block_number() >= 1 {
        // pallet_acurast_marketplace::on_finalize(System::block_number());
        Timestamp::on_finalize(System::block_number());
    }
    System::set_block_number(System::block_number() + 1);
    Timestamp::on_initialize(System::block_number());
}

/// A helper function to move time on in tests. It ensures `Timestamp::set` is only called once per block by advancing the block otherwise.
fn later(now: u64) {
    // If this is not the very first timestamp ever set, we always advance the block before setting new time
    // this is because setting it twice in a block is not legal
    if Timestamp::get() > 0 {
        // pretend block was finalized
        let b = System::block_number();
        next_block(); // we cannot set time twice in same block
        assert_eq!(b + 1, System::block_number());
    }
    // pretend time moved on
    assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
}
