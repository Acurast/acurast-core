#![cfg(test)]

use crate::chain::substrate::{MMRProofItems, ProofLeaf, SubstrateProof};
use crate::instances::AlephZeroInstance;
use crate::stub::*;
use crate::types::*;
use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
};
use frame_support::assert_ok;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{bounded_vec, AccountId32};
use std::marker::PhantomData;

#[test]
fn test_send_noop_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        let seq_id_before = 0;
        <crate::MessageSequenceId::<Test, AlephZeroInstance>>::set(seq_id_before);

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

        assert_ok!(AlephZeroHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "31c473f6554f31bd627e81973cc43a29cf560c1cff94f65ace807180659b5872"
        ));
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            AlephZeroHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(AlephZeroHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let proof: MMRProofItems = bounded_vec![
        ];
        let leaves: Vec<ProofLeaf> = bounded_vec![
                ProofLeaf {
                    leaf_index: 0,
                    data: hex!("0100000000000000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01000404").to_vec()
                }
        ];

        let proof = SubstrateProof::<AcurastAccountId, AccountId32> {
            mmr_size: 1,
            proof,
            leaves,
            marker: PhantomData::default()
        };

        assert_ok!(
            AlephZeroHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );
    });
}
