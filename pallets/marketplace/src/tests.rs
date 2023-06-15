#![cfg(test)]

use frame_support::{assert_err, assert_ok, traits::Hooks};
use pallet_acurast::MultiOrigin;
use sp_runtime::Permill;

use pallet_acurast::{
    utils::validate_and_extract_attestation, JobModules, JobRegistrationFor, Schedule,
};
use reputation::{BetaReputation, ReputationEngine};

use crate::{
    mock::*, AdvertisementRestriction, Assignment, Error, ExecutionResult, JobStatus, Match, SLA,
};
use crate::{stub::*, PubKeys};
use crate::{JobRequirements, PlannedExecution};

#[test]
fn test_match() {
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
            interval: 1_800_000,           // 30min
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: None,
            instant_match: None,
        },
    };
    let registration2 = JobRegistrationFor::<Test> {
        script: script(),
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration: 5000,
            start_time: 1_671_800_400_000, // 23.12.2022 13:00
            end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
            interval: 1_800_000,           // 30min
            max_start_delay: 10_000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: None,
            instant_match: None,
        },
    };

    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();

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
                allowed_consumers: ad.allowed_consumers.clone().map(|value| value.to_vec()),
                available_modules: JobModules::default(),
            }),
            AcurastMarketplace::stored_advertisement(processor_account_id())
        );
        assert_eq!(
            Some(ad.pricing.clone()),
            AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
        );

        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration1.clone(),
        ));
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration2.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(
                MultiOrigin::Acurast(alice_account_id()),
                initial_job_id + 1
            )
        );
        assert_eq!(
            Some(100_000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);
        let job_match1 = Match {
            job_id: job_id1.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        let job_id2 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 2);
        let job_match2 = Match {
            job_id: job_id2.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 5_000,
            }],
        };

        assert_ok!(AcurastMarketplace::propose_matching(
            RuntimeOrigin::signed(charlie_account_id()).into(),
            vec![job_match1.clone(), job_match2.clone()]
                .try_into()
                .unwrap(),
        ));
        assert_eq!(
            Some(JobStatus::Matched),
            AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
        );
        assert_eq!(
            Some(60_000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        assert_ok!(AcurastMarketplace::acknowledge_match(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id1.clone(),
            PubKeys::default(),
        ));
        assert_eq!(
            Some(JobStatus::Assigned(1)),
            AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
        );

        // pretend time moved on
        assert_eq!(1, System::block_number());
        later(registration1.schedule.start_time + 3000); // pretend actual execution until report call took 3 seconds
        assert_eq!(2, System::block_number());

        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id1.clone(),
            ExecutionResult::Success(operation_hash())
        ));
        // average reward only updated at end of job
        assert_eq!(None, AcurastMarketplace::average_reward());
        // reputation still ~50%
        assert_eq!(
            Permill::from_parts(509_803),
            BetaReputation::<u128>::normalize(
                AcurastMarketplace::stored_reputation(processor_account_id()).unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            Some(Assignment {
                slot: 0,
                start_delay: 0,
                fee_per_execution: 5_020_000,
                acknowledged: true,
                sla: SLA { total: 2, met: 1 },
                pub_keys: PubKeys::default(),
            }),
            AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()),
        );
        // Job still assigned after one execution
        assert_eq!(
            Some(JobStatus::Assigned(1)),
            AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1),
        );
        assert_eq!(
            Some(60000),
            AcurastMarketplace::stored_storage_capacity(processor_account_id())
        );

        // pretend time moved on
        later(registration1.schedule.range(0).unwrap().1 - 2000);
        assert_eq!(3, System::block_number());

        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id1.clone(),
            ExecutionResult::Success(operation_hash())
        ));

        // pretend time moved on
        later(registration1.schedule.end_time + 1);
        assert_eq!(4, System::block_number());

        assert_ok!(AcurastMarketplace::finalize_job(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id1.clone()
        ));

        assert_eq!(
            None,
            AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()),
        );
        assert_eq!(Some(2), AcurastMarketplace::total_assigned());
        // average reward only updated at end of job
        assert_eq!(Some(2510000), AcurastMarketplace::average_reward());
        // reputation increased
        assert_eq!(
            Permill::from_parts(763_424),
            BetaReputation::<u128>::normalize(
                AcurastMarketplace::stored_reputation(processor_account_id()).unwrap()
            )
            .unwrap()
        );
        // Job no longer assigned after last execution
        assert_eq!(
            None,
            AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1),
        );
        assert_eq!(
            // only job2 is still blocking memory
            Some(80_000),
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
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(12_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    job_id1.clone(),
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(12_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    job_id2.clone(),
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(job_match1)),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(job_match2)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(3_920_000)), // this is before splitting of the configured percentage that actually is transferred to the matcher
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssigned(
                    job_id1.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 0 },
                        pub_keys: PubKeys::default(),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(5_020_000)),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id1.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id1.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 1 },
                        pub_keys: PubKeys::default(),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(5_020_000)),
                RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
                    job_id1.clone(),
                    operation_hash()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::Reported(
                    job_id1.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 2 },
                        pub_keys: PubKeys::default(),
                    }
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobFinalized(job_id1.clone(),)),
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
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: None,
            instant_match: None,
        },
    };

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
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: None,
            instant_match: None,
        },
    };

    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();
        let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

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
            AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
        );

        // register second job
        assert_ok!(Acurast::register(
            RuntimeOrigin::signed(alice_account_id()).into(),
            registration2.clone(),
        ));
        assert_eq!(
            Some(JobStatus::Open),
            AcurastMarketplace::stored_job_status(&job_id1.0, job_id1.1 + 1)
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
            vec![m.clone()].try_into().unwrap(),
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
                vec![m.clone()].try_into().unwrap(),
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
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(12_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    job_id1.clone()
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(18_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration2.clone(),
                    (job_id1.0.clone(), &job_id1.1 + 1)
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(m)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(1_960_000)),
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
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: Some(1_000_000),
            instant_match: None,
        },
    };

    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();
        let job_id = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

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
            AcurastMarketplace::stored_job_status(&job_id.0, job_id.1)
        );

        // the job matches except inssufficient reputation
        let m = Match {
            job_id: job_id.clone(),
            sources: vec![PlannedExecution {
                source: processor_account_id(),
                start_delay: 0,
            }],
        };
        assert_err!(
            AcurastMarketplace::propose_matching(
                RuntimeOrigin::signed(charlie_account_id()).into(),
                vec![m.clone()].try_into().unwrap(),
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
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(12_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration1.clone(),
                    job_id.clone()
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
        required_modules: JobModules::default(),
        extra: JobRequirements {
            slots: 1,
            reward: 3_000_000 * 2,
            min_reputation: None,
            instant_match: None,
        },
    };

    ExtBuilder::default().build().execute_with(|| {
        let initial_job_id = Acurast::job_id_sequence();
        let job_id = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

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
                allowed_consumers: ad.allowed_consumers.clone().map(|value| value.to_vec()),
                available_modules: JobModules::default(),
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
            vec![m.clone()].try_into().unwrap(),
        ));

        assert_ok!(AcurastMarketplace::acknowledge_match(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            PubKeys::default(),
        ));

        // report twice with success
        // -------------------------

        // pretend time moved on
        let mut iter = registration.schedule.iter(0).unwrap();
        later(iter.next().unwrap() + 1000);
        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            ExecutionResult::Success(operation_hash())
        ));

        // pretend time moved on
        later(iter.next().unwrap() + 1000);
        assert_ok!(AcurastMarketplace::report(
            RuntimeOrigin::signed(processor_account_id()).into(),
            job_id.clone(),
            ExecutionResult::Success(operation_hash())
        ));

        // third report is illegal!
        later(registration.schedule.range(0).unwrap().1 + 1000);
        assert_err!(
            AcurastMarketplace::report(
                RuntimeOrigin::signed(processor_account_id()).into(),
                job_id.clone(),
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
                RuntimeEvent::MockPallet(mock_pallet::Event::Locked(12_000_000)),
                RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStored(
                    registration.clone(),
                    job_id.clone()
                )),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatched(m)),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayMatcherReward(1_960_000)),
                RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssigned(
                    job_id.clone(),
                    processor_account_id(),
                    Assignment {
                        slot: 0,
                        start_delay: 0,
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 0 },
                        pub_keys: PubKeys::default(),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(5_020_000)),
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
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 1 },
                        pub_keys: PubKeys::default(),
                    }
                )),
                RuntimeEvent::MockPallet(mock_pallet::Event::PayReward(5_020_000)),
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
                        fee_per_execution: 5_020_000,
                        acknowledged: true,
                        sla: SLA { total: 2, met: 2 },
                        pub_keys: PubKeys::default(),
                    }
                )),
            ]
        );
    });
}

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
