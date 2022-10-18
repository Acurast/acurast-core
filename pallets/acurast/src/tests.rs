#![cfg(test)]

use crate::{
    mock::*, utils::validate_and_extract_attestation, AllowedSourcesUpdate,
    CertificateRevocationListUpdate, Error, Fulfillment, ListUpdateOperation, SerialNumber,
};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use sp_runtime::MultiAddress;

#[test]
fn test_job_registration() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = job_registration(None, false);
        let register_call = Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        );
        assert_ok!(register_call);

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(alice_account_id(), registration.script.clone())
        );

        assert_ok!(Acurast::deregister(
            Origin::signed(alice_account_id()).into(),
            registration.script.clone()
        ));

        assert_eq!(
            None,
            Acurast::stored_job_registration(alice_account_id(), registration.script.clone())
        );

        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                Event::Acurast(crate::Event::JobRegistrationRemoved(
                    registration.script,
                    alice_account_id()
                ))
            ]
        );
    });
}

#[test]
fn test_job_registration_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = invalid_job_registration_1();
        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration.clone()
            ),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(alice_account_id(), registration.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = invalid_job_registration_2();
        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration.clone()
            ),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(alice_account_id(), registration.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let registration_1 = job_registration(
            Some(vec![
                alice_account_id(),
                bob_account_id(),
                charlie_account_id(),
                dave_account_id(),
                eve_account_id(),
            ]),
            false,
        );
        let registration_2 = job_registration(Some(vec![]), false);
        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration_1.clone()
            ),
            Error::<Test>::TooManyAllowedSources
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(alice_account_id(), registration_1.script)
        );

        assert_err!(
            Acurast::register(
                Origin::signed(alice_account_id()).into(),
                registration_2.clone()
            ),
            Error::<Test>::TooFewAllowedSources
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(alice_account_id(), registration_2.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_update_allowed_sources() {
    ExtBuilder::default().build().execute_with(|| {
        let registration_1 = job_registration(None, false);
        let registration_2 =
            job_registration(Some(vec![alice_account_id(), bob_account_id()]), false);
        let updates_1 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: alice_account_id(),
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: bob_account_id(),
            },
        ];
        let updates_2 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: alice_account_id(),
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: bob_account_id(),
            },
        ];
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration_1.clone(),
        ));

        assert_ok!(Acurast::update_allowed_sources(
            Origin::signed(alice_account_id()).into(),
            registration_1.script.clone(),
            updates_1.clone()
        ));

        assert_eq!(
            Some(registration_2.clone()),
            Acurast::stored_job_registration(alice_account_id(), &registration_1.script)
        );

        assert_ok!(Acurast::update_allowed_sources(
            Origin::signed(alice_account_id()).into(),
            registration_1.script.clone(),
            updates_2.clone()
        ));

        assert_eq!(
            Some(registration_1.clone()),
            Acurast::stored_job_registration(alice_account_id(), &registration_1.script)
        );

        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration_1.clone(),
                    alice_account_id()
                )),
                Event::Acurast(crate::Event::AllowedSourcesUpdated(
                    alice_account_id(),
                    registration_1,
                    updates_1
                )),
                Event::Acurast(crate::Event::AllowedSourcesUpdated(
                    alice_account_id(),
                    registration_2,
                    updates_2
                ))
            ]
        );
    });
}

#[test]
fn test_update_allowed_sources_failure() {
    let registration = job_registration(
        Some(vec![
            alice_account_id(),
            bob_account_id(),
            charlie_account_id(),
            dave_account_id(),
        ]),
        false,
    );
    let updates = vec![AllowedSourcesUpdate {
        operation: ListUpdateOperation::Add,
        account_id: eve_account_id(),
    }];
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));

        assert_err!(
            Acurast::update_allowed_sources(
                Origin::signed(alice_account_id()).into(),
                registration.script.clone(),
                updates.clone()
            ),
            Error::<Test>::TooManyAllowedSources
        );

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(alice_account_id(), &registration.script)
        );

        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
            ]
        );
    });
}

#[test]
fn test_assign_job() {
    let registration = job_registration(None, false);
    let updates = job_assignment_update_for(&registration, None);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_ok!(Acurast::update_job_assignments(
            Origin::signed(alice_account_id()),
            updates.clone(),
        ));

        assert_eq!(
            Some(vec![(alice_account_id(), registration.script.clone())]),
            Acurast::stored_job_assignment(processor_account_id())
        );

        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                Event::Acurast(crate::Event::JobAssignmentUpdate(
                    alice_account_id(),
                    updates
                )),
            ]
        );
    });
}

#[test]
fn test_assign_job_failure_1() {
    let registration = job_registration(None, true);
    let updates = job_assignment_update_for(&registration, None);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_err!(
            Acurast::update_job_assignments(
                Origin::signed(alice_account_id()).into(),
                updates.clone()
            ),
            Error::<Test>::FulfillSourceNotVerified
        );
        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
            ]
        );
    });
}

#[test]
fn test_assign_job_failure_2() {
    let registration = job_registration(Some(vec![charlie_account_id()]), false);
    let updates = job_assignment_update_for(&registration, None);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_err!(
            Acurast::update_job_assignments(Origin::signed(alice_account_id().into()), updates),
            Error::<Test>::FulfillSourceNotAllowed
        );
        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                ))
            ]
        );
    });
}

#[test]
fn test_fulfill() {
    let registration = job_registration(None, false);
    let fulfillment = Fulfillment {
        script: registration.script.clone(),
        payload: hex!("00").to_vec(),
    };
    let updates = job_assignment_update_for(&registration, None);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_ok!(Acurast::update_job_assignments(
            Origin::signed(alice_account_id()),
            updates.clone(),
        ));
        assert_ok!(Acurast::fulfill(
            Origin::signed(processor_account_id()).into(),
            fulfillment.clone(),
            MultiAddress::Id(alice_account_id()),
        ));
        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                Event::Acurast(crate::Event::JobAssignmentUpdate(
                    alice_account_id(),
                    updates
                )),
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: pallet_assets_account(),
                    to: processor_account_id(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::ReceivedFulfillment(
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
fn test_fulfill_failure_1() {
    let registration = job_registration(None, false);
    let fulfillment = Fulfillment {
        script: registration.script.clone(),
        payload: hex!("00").to_vec(),
    };
    let updates = job_assignment_update_for(&registration, None);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(alice_account_id()).into(),
            registration.clone(),
        ));
        assert_ok!(Acurast::update_job_assignments(
            Origin::signed(alice_account_id()),
            updates.clone(),
        ));
        assert_ok!(Acurast::deregister(
            Origin::signed(alice_account_id()).into(),
            registration.script.clone(),
        ));
        assert_err!(
            Acurast::fulfill(
                Origin::signed(processor_account_id()).into(),
                fulfillment.clone(),
                MultiAddress::Id(alice_account_id())
            ),
            Error::<Test>::JobRegistrationNotFound
        );
        assert_eq!(
            Some(vec![]),
            Acurast::stored_job_assignment(processor_account_id())
        );
        assert_eq!(
            events(),
            [
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: alice_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    alice_account_id()
                )),
                Event::Acurast(crate::Event::JobAssignmentUpdate(
                    alice_account_id(),
                    updates
                )),
                Event::Acurast(crate::Event::JobRegistrationRemoved(
                    registration.script,
                    alice_account_id()
                )),
            ],
        );
    });
}

#[test]
fn test_submit_attestation() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();
        let _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(processor_account_id()).into(),
            chain.clone()
        ));

        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();

        assert_eq!(
            Some(attestation.clone()),
            Acurast::stored_attestation(processor_account_id())
        );

        assert_eq!(
            events(),
            [Event::Acurast(crate::Event::AttestationStored(
                attestation,
                processor_account_id()
            ))]
        );
    });
}

#[test]
fn test_submit_attestation_register_fulfill() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();
        let registration = job_registration(None, true);
        let fulfillment = fulfillment_for(&registration);
        let updates = job_assignment_update_for(&registration, Some(bob_account_id()));
        let _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(processor_account_id()).into(),
            chain.clone()
        ));
        assert_ok!(Acurast::register(
            Origin::signed(bob_account_id()).into(),
            registration.clone()
        ));
        assert_ok!(Acurast::update_job_assignments(
            Origin::signed(bob_account_id().into()),
            updates.clone()
        ));
        assert_ok!(Acurast::fulfill(
            Origin::signed(processor_account_id()),
            fulfillment.clone(),
            MultiAddress::Id(bob_account_id())
        ));

        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();
        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::AttestationStored(
                    attestation,
                    processor_account_id()
                )),
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: bob_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    bob_account_id()
                )),
                Event::Acurast(crate::Event::JobAssignmentUpdate(bob_account_id(), updates)),
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: pallet_assets_account(),
                    to: processor_account_id(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::ReceivedFulfillment(
                    processor_account_id(),
                    fulfillment,
                    registration,
                    bob_account_id()
                )),
            ]
        );
    });
}

#[test]
fn test_submit_attestation_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = invalid_attestation_chain_1();

        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::CertificateChainTooShort
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        let chain = invalid_attestation_chain_2();

        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::RootCertificateValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        let chain = invalid_attestation_chain_3();

        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::CertificateChainValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        let _ = Timestamp::set(Origin::none(), 1657363914000);
        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        let _ = Timestamp::set(Origin::none(), 1842739199001);
        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(processor_account_id()));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_update_revocation_list() {
    ExtBuilder::default().build().execute_with(|| {
        let updates_1 = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            cert_serial_number: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            Origin::signed(alice_account_id()).into(),
            updates_1.clone(),
        ));
        assert_eq!(
            Some(()),
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        let updates_2 = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Remove,
            cert_serial_number: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            Origin::signed(alice_account_id()).into(),
            updates_2.clone(),
        ));
        assert_eq!(
            None,
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        assert_err!(
            Acurast::update_certificate_revocation_list(
                Origin::signed(bob_account_id()).into(),
                updates_1.clone(),
            ),
            Error::<Test>::CertificateRevocationListUpdateNotAllowed
        );
        assert_eq!(
            None,
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates_1
                )),
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates_2
                ))
            ]
        );
    });
}

#[test]
fn test_update_revocation_list_submit_attestation() {
    ExtBuilder::default().build().execute_with(|| {
        let updates = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            cert_serial_number: cert_serial_number(),
        }];
        assert_ok!(Acurast::update_certificate_revocation_list(
            Origin::signed(alice_account_id()).into(),
            updates.clone(),
        ));

        let chain = attestation_chain();
        let _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_err!(
            Acurast::submit_attestation(
                Origin::signed(processor_account_id()).into(),
                chain.clone()
            ),
            Error::<Test>::RevokedCertificate
        );

        assert_eq!(
            events(),
            [Event::Acurast(
                crate::Event::CertificateRecovationListUpdated(alice_account_id(), updates)
            ),]
        );
    });
}

#[test]
fn test_update_revocation_list_fulfill() {
    ExtBuilder::default().build().execute_with(|| {
        let updates = vec![CertificateRevocationListUpdate {
            operation: ListUpdateOperation::Add,
            cert_serial_number: cert_serial_number(),
        }];
        let chain = attestation_chain();
        let registration = job_registration(None, true);
        let assignment_updates = job_assignment_update_for(&registration, Some(bob_account_id()));
        let fulfillment = fulfillment_for(&registration);
        let _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(processor_account_id()).into(),
            chain.clone()
        ));
        assert_ok!(Acurast::register(
            Origin::signed(bob_account_id()).into(),
            registration.clone()
        ));
        assert_ok!(Acurast::update_job_assignments(
            Origin::signed(bob_account_id()),
            assignment_updates.clone()
        ));
        assert_ok!(Acurast::update_certificate_revocation_list(
            Origin::signed(alice_account_id()).into(),
            updates.clone(),
        ));
        assert_err!(
            Acurast::fulfill(
                Origin::signed(processor_account_id()),
                fulfillment.clone(),
                MultiAddress::Id(bob_account_id())
            ),
            Error::<Test>::RevokedCertificate
        );

        let attestation =
            validate_and_extract_attestation::<Test>(&processor_account_id(), &chain).unwrap();

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::AttestationStored(
                    attestation,
                    processor_account_id()
                )),
                Event::Assets(pallet_assets::Event::Transferred {
                    asset_id: 22,
                    from: bob_account_id(),
                    to: pallet_assets_account(),
                    amount: INITIAL_BALANCE / 2 as u128,
                }),
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    bob_account_id()
                )),
                Event::Acurast(crate::Event::JobAssignmentUpdate(
                    bob_account_id(),
                    assignment_updates
                )),
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(
                    alice_account_id(),
                    updates
                )),
            ]
        );
    });
}
