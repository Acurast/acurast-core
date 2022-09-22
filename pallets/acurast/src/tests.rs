#![cfg(test)]

use crate::{
    mock::*, utils::validate_and_extract_attestation, AllowedSourcesUpdate,
    CertificateRevocationListUpdate, Error, Fulfillment, ListUpdateOperation, SerialNumber,
};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;

#[test]
fn test_job_registration() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = job_registration(None, false);
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration.clone(),
        ));

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(1, registration.script.clone())
        );

        assert_ok!(Acurast::deregister(
            Origin::signed(1).into(),
            registration.script.clone()
        ));

        assert_eq!(
            None,
            Acurast::stored_job_registration(1, registration.script.clone())
        );

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 1)),
                Event::Acurast(crate::Event::JobRegistrationRemoved(registration.script, 1))
            ]
        );
    });
}

#[test]
fn test_job_registration_failure_1() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = invalid_job_registration_1();
        assert_err!(
            Acurast::register(Origin::signed(1).into(), registration.clone()),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(1, registration.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let registration = invalid_job_registration_2();
        assert_err!(
            Acurast::register(Origin::signed(1).into(), registration.clone()),
            Error::<Test>::InvalidScriptValue
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(1, registration.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_job_registration_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let registration_1 = job_registration(Some(vec![1, 2, 3, 4, 12]), false);
        let registration_2 = job_registration(Some(vec![]), false);
        assert_err!(
            Acurast::register(Origin::signed(1).into(), registration_1.clone()),
            Error::<Test>::TooManyAllowedSources
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(1, registration_1.script)
        );

        assert_err!(
            Acurast::register(Origin::signed(1).into(), registration_2.clone()),
            Error::<Test>::TooFewAllowedSources
        );

        assert_eq!(
            None,
            Acurast::stored_job_registration(1, registration_2.script)
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_update_allowed_sources() {
    ExtBuilder::default().build().execute_with(|| {
        let registration_1 = job_registration(None, false);
        let registration_2 = job_registration(Some(vec![1, 2]), false);
        let updates_1 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: 1,
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: 2,
            },
        ];
        let updates_2 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: 1,
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: 2,
            },
        ];
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration_1.clone(),
        ));

        assert_ok!(Acurast::update_allowed_sources(
            Origin::signed(1).into(),
            registration_1.script.clone(),
            updates_1.clone()
        ));

        assert_eq!(
            Some(registration_2.clone()),
            Acurast::stored_job_registration(1, &registration_1.script)
        );

        assert_ok!(Acurast::update_allowed_sources(
            Origin::signed(1).into(),
            registration_1.script.clone(),
            updates_2.clone()
        ));

        assert_eq!(
            Some(registration_1.clone()),
            Acurast::stored_job_registration(1, &registration_1.script)
        );

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::JobRegistrationStored(
                    registration_1.clone(),
                    1
                )),
                Event::Acurast(crate::Event::AllowedSourcesUpdated(
                    1,
                    registration_1,
                    updates_1
                )),
                Event::Acurast(crate::Event::AllowedSourcesUpdated(
                    1,
                    registration_2,
                    updates_2
                ))
            ]
        );
    });
}

#[test]
fn test_update_allowed_sources_failure() {
    let registration = job_registration(Some(vec![1, 2, 3, 4]), false);
    let updates = vec![AllowedSourcesUpdate {
        operation: ListUpdateOperation::Add,
        account_id: 12,
    }];
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration.clone(),
        ));

        assert_err!(
            Acurast::update_allowed_sources(
                Origin::signed(1).into(),
                registration.script.clone(),
                updates.clone()
            ),
            Error::<Test>::TooManyAllowedSources
        );

        assert_eq!(
            Some(registration.clone()),
            Acurast::stored_job_registration(1, &registration.script)
        );

        assert_eq!(
            events(),
            [Event::Acurast(crate::Event::JobRegistrationStored(
                registration.clone(),
                1
            )),]
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
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration.clone(),
        ));
        assert_ok!(Acurast::fulfill(
            Origin::signed(2).into(),
            fulfillment.clone(),
            1
        ));

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 1)),
                Event::Acurast(crate::Event::ReceivedFulfillment(
                    2,
                    fulfillment,
                    registration,
                    1
                )),
            ]
        );
    });
}

#[test]
fn test_fulfill_failure_1() {
    let fulfillment = Fulfillment {
        script: script(),
        payload: hex!("00").to_vec(),
    };
    ExtBuilder::default().build().execute_with(|| {
        assert_err!(
            Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
            Error::<Test>::JobRegistrationNotFound
        );

        assert_eq!(events(), []);
    });
}

#[test]
fn test_fulfill_failure_2() {
    let registration = job_registration(None, true);
    let fulfillment = fulfillment_for(&registration);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration.clone(),
        ));
        assert_err!(
            Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
            Error::<Test>::FulfillSourceNotVerified
        );

        assert_eq!(
            events(),
            [Event::Acurast(crate::Event::JobRegistrationStored(
                registration.clone(),
                1
            ))]
        );
    });
}

#[test]
fn test_fulfill_failure_3() {
    let registration = job_registration(Some(vec![3]), false);
    let fulfillment = fulfillment_for(&registration);
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Acurast::register(
            Origin::signed(1).into(),
            registration.clone(),
        ));
        assert_err!(
            Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
            Error::<Test>::FulfillSourceNotAllowed
        );

        assert_eq!(
            events(),
            [Event::Acurast(crate::Event::JobRegistrationStored(
                registration.clone(),
                1
            ))]
        );
    });
}

#[test]
fn test_submit_attestation() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();
        _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(1).into(),
            chain.clone()
        ));

        let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

        assert_eq!(Some(attestation.clone()), Acurast::stored_attestation(1));

        assert_eq!(
            events(),
            [Event::Acurast(crate::Event::AttestationStored(
                attestation,
                1
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

        _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(1).into(),
            chain.clone()
        ));
        assert_ok!(Acurast::register(
            Origin::signed(2).into(),
            registration.clone()
        ));
        assert_ok!(Acurast::fulfill(Origin::signed(1), fulfillment.clone(), 2));

        let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::AttestationStored(attestation, 1)),
                Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 2)),
                Event::Acurast(crate::Event::ReceivedFulfillment(
                    1,
                    fulfillment,
                    registration,
                    2
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
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::CertificateChainTooShort
        );

        assert_eq!(None, Acurast::stored_attestation(1));

        let chain = invalid_attestation_chain_2();

        assert_err!(
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::RootCertificateValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(1));

        let chain = invalid_attestation_chain_3();

        assert_err!(
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::CertificateChainValidationFailed
        );

        assert_eq!(None, Acurast::stored_attestation(1));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_2() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        _ = Timestamp::set(Origin::none(), 1657363914000);
        assert_err!(
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(1));

        assert_eq!(events(), []);
    });
}

#[test]
fn test_submit_attestation_failure_3() {
    ExtBuilder::default().build().execute_with(|| {
        let chain = attestation_chain();

        _ = Timestamp::set(Origin::none(), 1842739199001);
        assert_err!(
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::AttestationCertificateNotValid
        );

        assert_eq!(None, Acurast::stored_attestation(1));

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
            Origin::signed(1).into(),
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
            Origin::signed(1).into(),
            updates_2.clone(),
        ));
        assert_eq!(
            None,
            Acurast::stored_revoked_certificate::<SerialNumber>(cert_serial_number())
        );

        assert_err!(
            Acurast::update_certificate_revocation_list(
                Origin::signed(2).into(),
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
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates_1)),
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates_2))
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
            Origin::signed(1).into(),
            updates.clone(),
        ));

        let chain = attestation_chain();
        _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_err!(
            Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()),
            Error::<Test>::RevokedCertificate
        );

        assert_eq!(
            events(),
            [Event::Acurast(
                crate::Event::CertificateRecovationListUpdated(1, updates)
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
        let fulfillment = fulfillment_for(&registration);
        _ = Timestamp::set(Origin::none(), 1657363915001);
        assert_ok!(Acurast::submit_attestation(
            Origin::signed(1).into(),
            chain.clone()
        ));
        assert_ok!(Acurast::update_certificate_revocation_list(
            Origin::signed(1).into(),
            updates.clone(),
        ));
        assert_ok!(Acurast::register(
            Origin::signed(2).into(),
            registration.clone()
        ));
        assert_err!(
            Acurast::fulfill(Origin::signed(1), fulfillment.clone(), 2),
            Error::<Test>::RevokedCertificate
        );

        let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

        assert_eq!(
            events(),
            [
                Event::Acurast(crate::Event::AttestationStored(attestation, 1)),
                Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates)),
                Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 2)),
            ]
        );
    });
}
