#![cfg(test)]

use frame_support::{assert_err, assert_ok, error::BadOrigin};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::AccountId32;

use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
    Error,
};

pub fn alice_account_id() -> AccountId32 {
    [0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
    [1; 32].into()
}
pub const HASH: H256 = H256(hex!(
    "a3f18e4c6f0cdd0d8666f407610351cacb9a263678cf058294be9977b69f2cb3"
));

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
            actions
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
            actions.clone()
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
                actions
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
            actions
        ));

        System::set_block_number(9);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                5,
                HASH
            ),
            Error::<Test, ()>::SubmitOutsideTransmitterActivityWindow
        );

        System::set_block_number(10);
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(alice_account_id()),
            10,
            HASH
        ));

        System::set_block_number(19);
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(alice_account_id()),
            10,
            HASH
        ));

        System::set_block_number(20);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                5,
                HASH
            ),
            Error::<Test, ()>::SubmitOutsideTransmitterActivityWindow
        );
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
            actions
        ));

        System::set_block_number(10);
        assert_err!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                6,
                HASH
            ),
            Error::<Test, ()>::SubmitOutsideTransmitterActivityWindow
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
            actions
        ));

        System::set_block_number(10);

        // first submission for target chain block 10
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(alice_account_id()),
            10,
            HASH
        ));
        // does not validate until quorum reached
        assert_eq!(TezosHyperdrive::validate_state_merkle_root(10, HASH), false);

        // intermitted submission for different block is allowed!
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(bob_account_id()),
            15,
            HASH
        ));

        // second submission for target chain block 10
        assert_ok!(TezosHyperdrive::submit_state_merkle_root(
            RuntimeOrigin::signed(bob_account_id()),
            10,
            HASH
        ));
        // does validate since quorum reached
        assert_eq!(TezosHyperdrive::validate_state_merkle_root(10, HASH), true);

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
                    block: 10,
                    state_merkle_root: HASH
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootSubmitted {
                    block: 15,
                    state_merkle_root: HASH
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootSubmitted {
                    block: 10,
                    state_merkle_root: HASH
                }),
                RuntimeEvent::TezosHyperdrive(crate::Event::StateMerkleRootAccepted {
                    block: 10,
                    state_merkle_root: HASH
                })
            ]
        );
    });
}
