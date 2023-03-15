#![cfg(test)]

use frame_support::{assert_err, assert_ok, error::BadOrigin};

use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
    Error,
};

use crate::stub::*;
use crate::types::StateTransmitterUpdates;

#[test]
fn update_single_state_transmitters() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // A single action

        let actions = vec![StateTransmitterUpdate::Add(
            alice_account_id(),
            ActivityWindow {
                start_block: 0,
                end_block: 100,
            },
        )];

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        assert_eq!(
            events(),
            [RuntimeEvent::TezosHyperdrive(
                crate::Event::StateTransmittersUpdate {
                    added: vec![(
                        alice_account_id(),
                        ActivityWindow {
                            start_block: 0,
                            end_block: 100
                        }
                    )],
                    updated: vec![],
                    removed: vec![],
                }
            )]
        );
    });
}

#[test]
fn update_multiple_state_transmitters() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 0,
                    end_block: 100,
                },
            ),
            StateTransmitterUpdate::Update(
                alice_account_id(),
                ActivityWindow {
                    start_block: 0,
                    end_block: 100,
                },
            ),
            StateTransmitterUpdate::Remove(alice_account_id()),
        ];

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        assert_eq!(
            events(),
            [RuntimeEvent::TezosHyperdrive(
                crate::Event::StateTransmittersUpdate {
                    added: vec![(
                        alice_account_id(),
                        ActivityWindow {
                            start_block: 0,
                            end_block: 100
                        }
                    )],
                    updated: vec![(
                        alice_account_id(),
                        ActivityWindow {
                            start_block: 0,
                            end_block: 100
                        }
                    )],
                    removed: vec![(alice_account_id())],
                }
            )]
        );
    });
}

/// Non root calls should fail
#[test]
fn update_state_transmitters_non_root() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let actions = vec![StateTransmitterUpdate::Add(
            alice_account_id(),
            ActivityWindow {
                start_block: 0,
                end_block: 100,
            },
        )];

        assert_err!(
            TezosHyperdrive::update_state_transmitters(
                RuntimeOrigin::signed(alice_account_id()).into(),
                StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
            ),
            BadOrigin
        );
    });
}

#[test]
fn submit_outside_activity_window() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let actions = vec![StateTransmitterUpdate::Add(
            alice_account_id(),
            ActivityWindow {
                start_block: 10,
                end_block: 20,
            },
        )];

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        System::set_block_number(9);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                HASH
            ),
            Error::<Test, ()>::SubmitOutsideTransmitterActivityWindow
        );

        System::set_block_number(20);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                HASH
            ),
            Error::<Test, ()>::SubmitOutsideTransmitterActivityWindow
        );

        System::set_block_number(10);
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(alice_account_id()),
            1,
            HASH
        ));
    });
}

#[test]
fn submit_outside_transmission_rate() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let actions = vec![StateTransmitterUpdate::Add(
            alice_account_id(),
            ActivityWindow {
                start_block: 10,
                end_block: 20,
            },
        )];

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        System::set_block_number(10);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                6,
                HASH
            ),
            Error::<Test, ()>::UnexpectedSnapshot
        );
    });
}

#[test]
fn submit_state_merkle_root() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let actions = vec![
            StateTransmitterUpdate::Add(
                alice_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 20,
                },
            ),
            StateTransmitterUpdate::Add(
                bob_account_id(),
                ActivityWindow {
                    start_block: 10,
                    end_block: 50,
                },
            ),
        ];

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        System::set_block_number(10);

        // first submission for target chain snapshot 1
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(alice_account_id()),
            1,
            HASH
        ));
        // does not validate until quorum reached
        assert_eq!(TezosHyperdrive::validate_state_merkle_root(1, HASH), false);

        // intermitted submission for different snapshot is not allowed!
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                2,
                HASH
            ),
            Error::<Test, ()>::UnexpectedSnapshot
        );

        // second submission for target chain snapshot 1
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(bob_account_id()),
            1,
            HASH
        ));
        // does validate since quorum reached
        assert_eq!(TezosHyperdrive::validate_state_merkle_root(1, HASH), true);

        assert_eq!(
            events(),
            [
                RuntimeEvent::TezosHyperdrive(crate::Event::StateTransmittersUpdate {
                    added: vec![
                        (
                            alice_account_id(),
                            ActivityWindow {
                                start_block: 10,
                                end_block: 20
                            }
                        ),
                        (
                            bob_account_id(),
                            ActivityWindow {
                                start_block: 10,
                                end_block: 50
                            }
                        )
                    ],
                    updated: vec![],
                    removed: vec![],
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootSubmitted {
                    source: alice_account_id(),
                    snapshot: 1,
                    state_merkle_root: HASH
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootSubmitted {
                    source: bob_account_id(),
                    snapshot: 1,
                    state_merkle_root: HASH
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootAccepted {
                    snapshot: 1,
                    state_merkle_root: HASH
                })
            ]
        );
    });
}
