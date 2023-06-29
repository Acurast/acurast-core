#![cfg(test)]

use frame_support::{assert_err, assert_ok, error::BadOrigin};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::bounded_vec;
use sp_runtime::traits::Keccak256;

use crate::stub::*;
use crate::types::*;
use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
    Error,
};

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

#[test]
fn test_verify_proof() {
    let mut test = new_test_ext();

    let owner = StateOwner::try_from(
        hex!("050a0000001600009f7f36d0241d3e6a82254216d7de5780aa67d8f9").to_vec(),
    )
    .unwrap();
    let key = StateKey::try_from(hex!("0000000000000000000000000003e7").to_vec()).unwrap();
    let value = StateValue::try_from(hex!("0000000000000000000000000003e7").to_vec()).unwrap();

    test.execute_with(|| {
        let leaf = TezosHyperdrive::leaf_hash(owner, key, value);
        let proof: StateProof<H256> = bounded_vec![
            StateProofNode::Left(H256(hex!(
                "19520b9dd118ede4c96c2f12718d43e22e9c0412b39cd15a36b40bce2121ddff"
            ))),
            StateProofNode::Left(H256(hex!(
                "29ac39fe8a6f05c0296b2f57769dae6a261e75a668c5b75bb96f43426e738a7d"
            ))),
            StateProofNode::Right(H256(hex!(
                "7e6f448ed8ceff132d032cc923dcd3f49fa7e702316a3db73e09b1ba2beea812"
            ))),
            StateProofNode::Left(H256(hex!(
                "47811eb10e0e7310f8e6c47b736de67b9b68f018d9dc7a224a5965a7fe90d405"
            ))),
            StateProofNode::Right(H256(hex!(
                "7646d25d9a992b6ebb996c2c4e5530ffc18f350747c12683ce90a1535305859c"
            ))),
            StateProofNode::Right(H256(hex!(
                "fe9181cc5392bc544a245964b1d39301c9ebd75c2128765710888ba4de9e61ea"
            ))),
            StateProofNode::Right(H256(hex!(
                "12f6db53d79912f90fd2a58ec4c30ebd078c490a6c5bd68c32087a3439ba111a"
            ))),
            StateProofNode::Right(H256(hex!(
                "efac0c32a7c7ab5ee5140850b5d7cbd6ebfaa406964a7e1c10239ccb816ea75e"
            ))),
            StateProofNode::Left(H256(hex!(
                "ceceb700876e9abc4848969882032d426e67b103dc96f55eeab84f773a7eeb5c"
            ))),
            StateProofNode::Left(H256(hex!(
                "abce2c418c92ca64a98baf9b20a3fcf7b5e9441e1166feedf4533b57c4bfa6a4"
            ))),
        ];

        let root_hash: H256 = H256(hex!(
            "fd5f82b627a0b2c5ac0022a95422d435b204c4c1071d5dbda84ae8708d0110fd"
        ));
        assert_eq!(derive_proof::<Keccak256, _>(proof, leaf), root_hash);
    });
}

#[test]
fn test_send_message_value_parsing_fails() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let seq_id_before = TezosHyperdrive::message_seq_id();

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

        let tezos_contract = StateOwner::try_from(hex!("050a00000016011ba8a95352a4d7f3c753ca700e10ab46cbf963f400").to_vec()).unwrap();
        assert_ok!(TezosHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            tezos_contract.clone()
        ));

        assert_eq!(TezosHyperdrive::current_target_chain_owner(), tezos_contract);

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "bed0c00cb1d8727772702af88a395a4e6c82ac6230cc1daf0610d97470377b91"
        ));
        assert_ok!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(TezosHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: StateProof<H256> = bounded_vec![];
        let key = StateKey::try_from(hex!("050001").to_vec()).unwrap();
        let value = StateValue::try_from(hex!("050707010000000c52454749535445525f4a4f4207070a0000001600008a8584be3718453e78923713a6966202b05f99c60a000000ee05070703030707050902000000250a00000020000000000000000000000000000000000000000000000000000000000000000007070707000007070509020000002907070a00000020111111111111111111111111111111111111111111111111111111111111111100000707030607070a00000001ff00010707000107070001070700010707020000000200000707070700b0d403070700b4f292aaf36107070098e4030707000000b4b8dba6f36107070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f374144583263644465610001").to_vec()).unwrap();

        assert_ok!(
            TezosHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof,
                key,
                value
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(TezosHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::TezosHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ParsingValueFailed)),
        );
    });
}

#[test]
fn test_send_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 74;
        <crate::MessageSequenceId::<Test>>::set(seq_id_before);


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

        let tezos_contract = StateOwner::try_from(hex!("050a000000160199651cbe1a155a5c8e5af7d6ea5c3f48eebb8c9c00").to_vec()).unwrap();
        assert_ok!(TezosHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            tezos_contract.clone()
        ));

        assert_eq!(TezosHyperdrive::current_target_chain_owner(), tezos_contract);

        assert_ok!(TezosHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));

        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "8303857bb23c1b072d9b52409fffe7cf6de57c33b2776c7de170ec94d01f02fc"
        ));
        assert_ok!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            TezosHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(TezosHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: StateProof<H256> = bounded_vec![];
        let key = StateKey::try_from(hex!("05008b01").to_vec()).unwrap();
        let value = StateValue::try_from(hex!("050707010000000c52454749535445525f4a4f4207070a00000016000016e64994c2ddbd293695b63e4cade029d3c8b5e30a000000ec050707030a0707050902000000250a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f070707070509020000002907070a00000020d80a8b0d800a3320528693947f7317871b2d51e5f3c8f3d0d4e4f7e6938ed68f00000707050900000707008080e898a9bf8d0700010707001d0707000107070001070702000000000707070700b40707070080cfb1eca062070700a0a9070707000000a0a5aaeca06207070a00000035697066733a2f2f516d536e317252737a444b354258634e516d4e367543767a4d376858636548555569426b61777758396b534d474b0000").to_vec()).unwrap();

        assert_ok!(
            TezosHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof,
                key,
                value
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(TezosHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::TezosHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}
