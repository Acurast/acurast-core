#![cfg(test)]

use frame_support::assert_ok;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{bounded_vec, AccountId32};
use std::marker::PhantomData;

use crate::chain::ethereum::{
    EthereumProof, EthereumProofItem, EthereumProofItems, EthereumProofValue,
};
use crate::instances::EthereumInstance;
use crate::stub::*;
use crate::types::*;
use crate::{
    mock::*,
    types::{ActivityWindow, StateTransmitterUpdate},
};

#[test]
fn test_send_register_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 0;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

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

        let ethereum_contract = StateOwner::try_from(hex!("57e796ff645719FC98b9Be0b3731F1605Ed98c5e").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "165f651aca44dc76ac642127d4a904b2270b22459c13b1bd5a360ea25f314f1d"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0dded1ea1ded6da9ae53a491896a34344c885a87123a0fc3406403c40dada8fe1a062fad4c414d0968b0d4b61b06abe4a12785c05dc94f2052087a24603377a1127a0762dbe56774d5c2f74f3bc62eebbff2c1ef8d622e044d763ef46a9c6e05b0895a024e16ad0a70f8e2a373cd20fbcb1beefd236930c9e80972676ee75e71376f748a05c0b8e998808ff95805f6e6d114aca2bdbc3c8a05cac100caa83453a80fc551da084cc118be70d76f1d7d45e731adb32a5ea98194f1e55e12e8a81c1265bb35804a0cfe0d0d5a659bf87435d9175311ce312a99fde37a714844fe8a86b6671485b29a014d0b6e6bd843f8f5652a691b07e8bd57d8daefebf68518dfa1ca2c27371c955a0e59a89693faa726341f6eac51e893e81bcdf3983788bfcf88cd35e85f2692072a074ad7d620707530ae12cef2d83f28cfa6b381176f2f17dda157f3d94194c3edaa060e8042b25a70cbdb5b7bf7acd57e96f389f8c5ca61568a3d104bb6e89410397a00be1c2a477f8eac3a4ba620b959b4e9702f919ef860337420a3dce5be814158fa091dff9cd84917e1dad5c65c824aa549be1e2b537eaa84adebbf0c7a34c0516b8a09db42f5adcc060c2fb368c2b5b5f2df2bd432018e76c5497305cf9ab040ece2fa0827453674aa8a0721788852fc341b097c451b87949198314a6df7bf7640c34bea0926595591284d182861c26ea9e71e86c7b9f8ee3b6859ee29113ebe08026916b80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ee8f768253ed6c37765832d64884a5e092e41cbfdaf0fb8f0932211ce0d568c4a0ce495a7fa0836aaeab91fbbdc7dea5ce1aa96b857fd1a8e39f86f254010f2973a0491069c53daf75d1997636cf24365e3df2fb7b0c1f473512f4015bccd7c0c495a0bc5e1a2ff02cee76141ebbde7e11ef1cde35ea2870dfb9c86c3686a1149359d3a0981b41642f7de62e3b86aacbaf87550fd5e6cd25fa74184b83547adf23a22fc5a0fddadd87475a00c34bb9d6f0247c6f2e1aa35a9ae3f704e053a8a5c2ce5c3913a073f979076c82f9b273fdb1490eed2a42118a22657e9e4aff5531b9434b601bf1a034f4c189b0d5b87be223f8d9ded6ea924b183ee5cf6559406de3c8c0a748df0fa0ad24cc8adf2c208cd3b4a618b16bbf9ad14caacd2b07fbc90bb62df6be85496ea0ea35325f678defc5ea53d77a23a293fc68c2d0b39fb1231466cd6c234da540b8a00ba70a69e36fa88db1d0eaa49c5b55e898560a8f7eefe11b6784020be2dbd8f4a0b3caf50d4308581d338e3ea7c22525d82188aebbee020b632587460edeae4af8a02272da0b5d0a15df06e7afbcb3c72d46f014ff8ffc767fed67bd913f7726681da0b7e7835994dcd6ff3b2acb150d385946911e12cff2deebce7b2c1d330443812fa0eaf2a4492c71a0c4f472d0def98e87407968800bef137205774089c5f516fcd4a093473cfe514cb3d370ab4be46ac4e5f4d7789246018164ce27b0aa67f389665e80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a03467744e52388cea47422c20dd914ba06b7774b831b6495bb4698c15d389148ba088525a844b5a5d4d8ef78f2a3778ed158134c67bf398c7d3bdf75eaeecaea572a05eb80f9c3226203a10218dc0fae5ae105405d88965dae3f54e8182a78cce661ca0fce6fb480556411a6154d06549bb13faead17c58ad5a2f007dd15c27c8b71ab9a039ce7123fca637f72e2453bf5fb6d41d339c9ed89d952528b2834864b3b95380a049082b9365e9191eae486a26eb45c46b86c684f1e681262ab2901e8966f00e9da0d4033860fd491626ee7b19e5a139740adb20b20681a4caa3476d192bbe0fb394a0c93505a507ac2a8c02679f893abd01c472a1d4f55da00fdc761d8b05ec3af045a0974c124e54de2d0d35da4e8f96e0e79594f1fb8c89eb9988dcce906ba03000a4a0d2b3c203d5ec947f1a8fb4d1bb9d0c988543f26aabbc41348e539a1611b2f1dda08829379b6999f03962ed414b65a0a23eb969525a0ad183182c32b116cc889be1a0721b85e9bb81dba6362be176d5a335ed706ba83d0e977ef7621d72060d84e28fa0174f7dc9393883bcb4439c7af8b090e45edd7e812908bd8ff47903f371471fc3a0713ab60afd75ae70440f07eac886187729e27b06f760f056e81cf2ea72b766f0a0197b0c5479eb956a72b7b52e5b4a6c2617afb71d7437559f208850d3b2fac728a0d8a6b21e99114d2bcf3ccc468bd6bfd3c6cef707c930b709294e90b537d20ead80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ff0a1740e1d2306fe57d45a2a282ac854ba247fef3d5b2bc22e83ee38a381578a0d2e14547273fc537625a75c302f56ee66cebc4768a150cfe58001566d7aa2d7ca010ee81a990481ea9d551664be71e8d9151dc05c2e11f9580f744b4c2a797ebfca037d54c4a3a414ce5b1dd5b05f54c3f9eb188a83f211d10e9856f3aee9fbe6c13a0afd0cb95be49860f9374c39afdb6bfff7bb920c1fa2b43713a62496198756b97a0316aa0c4bab2b1863dbfa2ec6ebadb68177d2af31c9801320c7ddfd26837a754a09a22b6f4e8b3bc901cdf1904c52a74f60e467521b71571c8919291e38ce646d4a09a67f363de88b930c48910acfe09799e88ad844ddbfe13cd644e755e99007694a0d0ebf268b32959711320592556273d90a191832efb32dad8313e454ac89b1203a00fbd113c8f88fa023ef525f1e963ba11cb31926bfd8e78e4a3d3a494cf011da6a086e5af77f6983dff46261e4ae6b04b26af984341fdb8820371b209c07baa8b31a007905fa7cfd6e1e359772711455b981134fc3a61748229d8a8fd0e38e7b4345aa077948f4f9040d14cfbfb4e6a4d998665c2284e6e613f5d6cd66e5b34e58fd7f9a03b86f28bc11cee028ed3b69fc2561ccb55b8563225537db8fce33cd7346b52cca015ca6665e74023a3d81068daf162f1dcd99880d0d0b9c4c08bc7a9011bcbe31ea051df22083c50d8216e7ae63ff8386c8e013ebfe146ac088cc068e8a98fbf5e2080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0f8e2caf4bfbd2387b011ae9b4e0434c4ef6c17fb7104bd3851ca9aa14c4c7be3a0b206684c96b45284dc843db0ef51474ac844697367d1e5f7a4b09b1d5b88b158a075a3fa28669ab58184a2e5bcd13eb55bf4751cfb4860c8074ff70442494d2229a019045885703bff231764c236c2dee89caee45ada51cebc30b8c2efe81a9f3782a0f453685df20ca90e4d1d642f1473139fff91f4436902baae4e704ad26976fc7ea05001ac402c71ec49abe1c679a5c6fd4fa0e55f49c597bec9d76de9cbf3ff5c08a013a1e86fe06edd27811e5129ea1730110de04bbd34e9a7bee5b717ec0023b78fa0a5e1c975d1f3ccb82efb8fbb767d9ce88851e034d9744837d59ca93b3f6c685ba07a0663e7103382619141edebc678ad5dedadb7aaf85c58289601fd583a35812aa0a15e7842d62674143b0e47ec04e5112e67d45ae9f89abdf73a421f0693a782e7a027b21a1eb9d192bfc7c24029690a3ccbe16771b82830dbe86ebf8e1be58aa8a0a0331021dc66a180e92ac2e1612db10bef58e6b79e86c1823c2dd5a906240a0930a0cb5bfa5dd659f5b0fb20ef8cc9471fa0abf204109e5486320f58d0d930c3c77ca0a0f05d5b9e5364abc8803976ba186997edafbac57fd64fa92a653010f62415b5a0001b4772a49facffbfd536e0e2dd12140dd71adcdf5a7e6d489e329b061b2585a0a029809ef5d8d359bd8f7fecf169578d49c6010a64b32c8649bd46e1bbfa65ea80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90151a06801d40737cad61186b939eb4104c5502273d26c535f7a37c34a63abf6e4dfe980a0b31f02a6b752f8eba131c4ec775ce0444ad2857792d19ccf587726dce76a273280a0027f124239b915e70ef1a986bfe2dfac456fd44237382307eb754ecd44e3821280a036ae5c8e0cc6004f5d1d4c66c236f7930ba2f64cbae6d9f0363876e47a01445aa0afd1c866b64cf9611d0c3989dad1a383fc035cb72c0bcd0882444d77ecbe0cf18080a0fa1915f63b7faf7d5b2a76aaeeaa9a61a7ed451c1f4718e8bf3e5af77581eac8a0e91198c91dcacf9d20eaadff1f6532676f2e377287e68c88b043d2aefe8e9119a096f9d03774a23f2570ffb8cc4298ffc267f4364bc746069975f21c45726fc71a80a00442927e748e876312dea52b28dfccfcf0b18b3daa10e62a317c40e858dbb201a0e88950e7888948366eb14c601bbc63a1ca546fba0aedfa5a38df928e5ee7982f80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f85180808080808080a084e54b0b5c476fbaae3840e18ea5f1b54d2cbfd4b5d11c3945113253b2966836808080808080a0e71dfd63f0c4ffd6568e8230d4ab61ff8a344ee88295aa8da2c6a5eea96960288080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d3eba0fa7e2848bcd30e1bf958707d2a02f8f03ae438d6622a21b562c7fb846f8440180a0ab4f5e5ac89f9bed9eab40a5b02763168e73ca32c5dd9f5ced76ae92815e42e1a03c789e7c0b32cfb991ed499ea05ca68b7ae8e89f5ed2bbf04deb5878c4018f68").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a04d6190501267fd3b5a9d1223d003b4bf5ae76063514a1033389b366426814821a070bf5d648f0dc17e554de24b7f1b7c803652abc65f216feaca4e1173eb334b10a079e72bd9aac24a2713cd23c2e3e838573073440d39c696b37b9a00343c590433a0d0dfa426aabf2e1cc49d1c4e990de5b4acf44d45337c2ffbd7403e9f3e76dd2ca0e03f9e0f2f37b58194656eda176816309348200438b94ed468f49f9b52539003a0fbb5f5c46cfbbec7dfaca1ab5bb383bb53f1f4b92ac955b7577c79877fb4a757a09eabfc1cafe877b5a9c85323492bf327c6be4ba87c0e575312f8a3ac0dd11707a09fe41785607793ac153fc5fba450a0ce95778bf772d42cb6514f152624622ab8a02018a6ee674122dffe10c2c7bfc53d032db313f05ea83fb254735f419611a37ba0d7a47dc607220e44942fa5f4c0475ae561777f2b13481d019dc69d474aab9f35a01d93a510fea6b836e1a07a7ce7dcaf42c46f60d2b136e0183e43d833069c36afa0a0d044a21761c2fd8ef694e0e06ccfb0d27f51d99049727fd06237b43835ad0fa00116e6173243473e292370aa7429b693bb8527192fe171dc25e642ddac35c42da0e0f4f15f3f45246a720e396433f58d0a5efcc3bd2c41ea4c1a9e7f91fdd03f88a068eb77e2f812d622596e51c1cb7ba59e9ea9984c9d692a615ec3b75705a6c386a09afddeaad8be58b213f1c59683b25729c4224d94d1d578954331416a8320113a80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8b180a0decfb671ca9f5f4a9435a00b7c90938fd5161d040f0e4ec2bef9fcda058fb7e58080808080a0307f066f3c48f0bafa465d4ff4c8319a419ab9e39513012ccf821c7fd8c997bd80a06bcbf785e8a691978458bd35eff03717df42067a74ac4f1e38e27c03ce263a2ca0cae3f457040328afaa28c753bdb0a2f57f06ba9bf265884c98877b6afc88ae3b808080a06df688a9c9a2d6e74276992a37a59de662a2ba20e9cff4d98faae2701d7231a08080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a020b3c568e6b9e23c87101e15642000038e2a634c0eba9355f868407d119483c2a1a012d978c1d3a38da7ff6eaa2e2202e47b156dbb8c208c2df40d4a2596d1ec50fb").to_vec()).unwrap(),
        ];
        let message_id = 1u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000001c0000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000001e0000000000000000000000000000000000000000000000000000000000000028000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000").to_vec()).unwrap();

        let proof = EthereumProof::<AcurastAccountId, AccountId32> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_deregister_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 1;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

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

        let ethereum_contract = StateOwner::try_from(hex!("57e796ff645719FC98b9Be0b3731F1605Ed98c5e").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "165f651aca44dc76ac642127d4a904b2270b22459c13b1bd5a360ea25f314f1d"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0dded1ea1ded6da9ae53a491896a34344c885a87123a0fc3406403c40dada8fe1a062fad4c414d0968b0d4b61b06abe4a12785c05dc94f2052087a24603377a1127a0762dbe56774d5c2f74f3bc62eebbff2c1ef8d622e044d763ef46a9c6e05b0895a024e16ad0a70f8e2a373cd20fbcb1beefd236930c9e80972676ee75e71376f748a05c0b8e998808ff95805f6e6d114aca2bdbc3c8a05cac100caa83453a80fc551da084cc118be70d76f1d7d45e731adb32a5ea98194f1e55e12e8a81c1265bb35804a0cfe0d0d5a659bf87435d9175311ce312a99fde37a714844fe8a86b6671485b29a014d0b6e6bd843f8f5652a691b07e8bd57d8daefebf68518dfa1ca2c27371c955a0e59a89693faa726341f6eac51e893e81bcdf3983788bfcf88cd35e85f2692072a074ad7d620707530ae12cef2d83f28cfa6b381176f2f17dda157f3d94194c3edaa060e8042b25a70cbdb5b7bf7acd57e96f389f8c5ca61568a3d104bb6e89410397a00be1c2a477f8eac3a4ba620b959b4e9702f919ef860337420a3dce5be814158fa091dff9cd84917e1dad5c65c824aa549be1e2b537eaa84adebbf0c7a34c0516b8a09db42f5adcc060c2fb368c2b5b5f2df2bd432018e76c5497305cf9ab040ece2fa0827453674aa8a0721788852fc341b097c451b87949198314a6df7bf7640c34bea0926595591284d182861c26ea9e71e86c7b9f8ee3b6859ee29113ebe08026916b80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ee8f768253ed6c37765832d64884a5e092e41cbfdaf0fb8f0932211ce0d568c4a0ce495a7fa0836aaeab91fbbdc7dea5ce1aa96b857fd1a8e39f86f254010f2973a0491069c53daf75d1997636cf24365e3df2fb7b0c1f473512f4015bccd7c0c495a0bc5e1a2ff02cee76141ebbde7e11ef1cde35ea2870dfb9c86c3686a1149359d3a0981b41642f7de62e3b86aacbaf87550fd5e6cd25fa74184b83547adf23a22fc5a0fddadd87475a00c34bb9d6f0247c6f2e1aa35a9ae3f704e053a8a5c2ce5c3913a073f979076c82f9b273fdb1490eed2a42118a22657e9e4aff5531b9434b601bf1a034f4c189b0d5b87be223f8d9ded6ea924b183ee5cf6559406de3c8c0a748df0fa0ad24cc8adf2c208cd3b4a618b16bbf9ad14caacd2b07fbc90bb62df6be85496ea0ea35325f678defc5ea53d77a23a293fc68c2d0b39fb1231466cd6c234da540b8a00ba70a69e36fa88db1d0eaa49c5b55e898560a8f7eefe11b6784020be2dbd8f4a0b3caf50d4308581d338e3ea7c22525d82188aebbee020b632587460edeae4af8a02272da0b5d0a15df06e7afbcb3c72d46f014ff8ffc767fed67bd913f7726681da0b7e7835994dcd6ff3b2acb150d385946911e12cff2deebce7b2c1d330443812fa0eaf2a4492c71a0c4f472d0def98e87407968800bef137205774089c5f516fcd4a093473cfe514cb3d370ab4be46ac4e5f4d7789246018164ce27b0aa67f389665e80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a03467744e52388cea47422c20dd914ba06b7774b831b6495bb4698c15d389148ba088525a844b5a5d4d8ef78f2a3778ed158134c67bf398c7d3bdf75eaeecaea572a05eb80f9c3226203a10218dc0fae5ae105405d88965dae3f54e8182a78cce661ca0fce6fb480556411a6154d06549bb13faead17c58ad5a2f007dd15c27c8b71ab9a039ce7123fca637f72e2453bf5fb6d41d339c9ed89d952528b2834864b3b95380a049082b9365e9191eae486a26eb45c46b86c684f1e681262ab2901e8966f00e9da0d4033860fd491626ee7b19e5a139740adb20b20681a4caa3476d192bbe0fb394a0c93505a507ac2a8c02679f893abd01c472a1d4f55da00fdc761d8b05ec3af045a0974c124e54de2d0d35da4e8f96e0e79594f1fb8c89eb9988dcce906ba03000a4a0d2b3c203d5ec947f1a8fb4d1bb9d0c988543f26aabbc41348e539a1611b2f1dda08829379b6999f03962ed414b65a0a23eb969525a0ad183182c32b116cc889be1a0721b85e9bb81dba6362be176d5a335ed706ba83d0e977ef7621d72060d84e28fa0174f7dc9393883bcb4439c7af8b090e45edd7e812908bd8ff47903f371471fc3a0713ab60afd75ae70440f07eac886187729e27b06f760f056e81cf2ea72b766f0a0197b0c5479eb956a72b7b52e5b4a6c2617afb71d7437559f208850d3b2fac728a0d8a6b21e99114d2bcf3ccc468bd6bfd3c6cef707c930b709294e90b537d20ead80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ff0a1740e1d2306fe57d45a2a282ac854ba247fef3d5b2bc22e83ee38a381578a0d2e14547273fc537625a75c302f56ee66cebc4768a150cfe58001566d7aa2d7ca010ee81a990481ea9d551664be71e8d9151dc05c2e11f9580f744b4c2a797ebfca037d54c4a3a414ce5b1dd5b05f54c3f9eb188a83f211d10e9856f3aee9fbe6c13a0afd0cb95be49860f9374c39afdb6bfff7bb920c1fa2b43713a62496198756b97a0316aa0c4bab2b1863dbfa2ec6ebadb68177d2af31c9801320c7ddfd26837a754a09a22b6f4e8b3bc901cdf1904c52a74f60e467521b71571c8919291e38ce646d4a09a67f363de88b930c48910acfe09799e88ad844ddbfe13cd644e755e99007694a0d0ebf268b32959711320592556273d90a191832efb32dad8313e454ac89b1203a00fbd113c8f88fa023ef525f1e963ba11cb31926bfd8e78e4a3d3a494cf011da6a086e5af77f6983dff46261e4ae6b04b26af984341fdb8820371b209c07baa8b31a007905fa7cfd6e1e359772711455b981134fc3a61748229d8a8fd0e38e7b4345aa077948f4f9040d14cfbfb4e6a4d998665c2284e6e613f5d6cd66e5b34e58fd7f9a03b86f28bc11cee028ed3b69fc2561ccb55b8563225537db8fce33cd7346b52cca015ca6665e74023a3d81068daf162f1dcd99880d0d0b9c4c08bc7a9011bcbe31ea051df22083c50d8216e7ae63ff8386c8e013ebfe146ac088cc068e8a98fbf5e2080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0f8e2caf4bfbd2387b011ae9b4e0434c4ef6c17fb7104bd3851ca9aa14c4c7be3a0b206684c96b45284dc843db0ef51474ac844697367d1e5f7a4b09b1d5b88b158a075a3fa28669ab58184a2e5bcd13eb55bf4751cfb4860c8074ff70442494d2229a019045885703bff231764c236c2dee89caee45ada51cebc30b8c2efe81a9f3782a0f453685df20ca90e4d1d642f1473139fff91f4436902baae4e704ad26976fc7ea05001ac402c71ec49abe1c679a5c6fd4fa0e55f49c597bec9d76de9cbf3ff5c08a013a1e86fe06edd27811e5129ea1730110de04bbd34e9a7bee5b717ec0023b78fa0a5e1c975d1f3ccb82efb8fbb767d9ce88851e034d9744837d59ca93b3f6c685ba07a0663e7103382619141edebc678ad5dedadb7aaf85c58289601fd583a35812aa0a15e7842d62674143b0e47ec04e5112e67d45ae9f89abdf73a421f0693a782e7a027b21a1eb9d192bfc7c24029690a3ccbe16771b82830dbe86ebf8e1be58aa8a0a0331021dc66a180e92ac2e1612db10bef58e6b79e86c1823c2dd5a906240a0930a0cb5bfa5dd659f5b0fb20ef8cc9471fa0abf204109e5486320f58d0d930c3c77ca0a0f05d5b9e5364abc8803976ba186997edafbac57fd64fa92a653010f62415b5a0001b4772a49facffbfd536e0e2dd12140dd71adcdf5a7e6d489e329b061b2585a0a029809ef5d8d359bd8f7fecf169578d49c6010a64b32c8649bd46e1bbfa65ea80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90151a06801d40737cad61186b939eb4104c5502273d26c535f7a37c34a63abf6e4dfe980a0b31f02a6b752f8eba131c4ec775ce0444ad2857792d19ccf587726dce76a273280a0027f124239b915e70ef1a986bfe2dfac456fd44237382307eb754ecd44e3821280a036ae5c8e0cc6004f5d1d4c66c236f7930ba2f64cbae6d9f0363876e47a01445aa0afd1c866b64cf9611d0c3989dad1a383fc035cb72c0bcd0882444d77ecbe0cf18080a0fa1915f63b7faf7d5b2a76aaeeaa9a61a7ed451c1f4718e8bf3e5af77581eac8a0e91198c91dcacf9d20eaadff1f6532676f2e377287e68c88b043d2aefe8e9119a096f9d03774a23f2570ffb8cc4298ffc267f4364bc746069975f21c45726fc71a80a00442927e748e876312dea52b28dfccfcf0b18b3daa10e62a317c40e858dbb201a0e88950e7888948366eb14c601bbc63a1ca546fba0aedfa5a38df928e5ee7982f80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f85180808080808080a084e54b0b5c476fbaae3840e18ea5f1b54d2cbfd4b5d11c3945113253b2966836808080808080a0e71dfd63f0c4ffd6568e8230d4ab61ff8a344ee88295aa8da2c6a5eea96960288080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d3eba0fa7e2848bcd30e1bf958707d2a02f8f03ae438d6622a21b562c7fb846f8440180a0ab4f5e5ac89f9bed9eab40a5b02763168e73ca32c5dd9f5ced76ae92815e42e1a03c789e7c0b32cfb991ed499ea05ca68b7ae8e89f5ed2bbf04deb5878c4018f68").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a04d6190501267fd3b5a9d1223d003b4bf5ae76063514a1033389b366426814821a070bf5d648f0dc17e554de24b7f1b7c803652abc65f216feaca4e1173eb334b10a079e72bd9aac24a2713cd23c2e3e838573073440d39c696b37b9a00343c590433a0d0dfa426aabf2e1cc49d1c4e990de5b4acf44d45337c2ffbd7403e9f3e76dd2ca0e03f9e0f2f37b58194656eda176816309348200438b94ed468f49f9b52539003a0fbb5f5c46cfbbec7dfaca1ab5bb383bb53f1f4b92ac955b7577c79877fb4a757a09eabfc1cafe877b5a9c85323492bf327c6be4ba87c0e575312f8a3ac0dd11707a09fe41785607793ac153fc5fba450a0ce95778bf772d42cb6514f152624622ab8a02018a6ee674122dffe10c2c7bfc53d032db313f05ea83fb254735f419611a37ba0d7a47dc607220e44942fa5f4c0475ae561777f2b13481d019dc69d474aab9f35a01d93a510fea6b836e1a07a7ce7dcaf42c46f60d2b136e0183e43d833069c36afa0a0d044a21761c2fd8ef694e0e06ccfb0d27f51d99049727fd06237b43835ad0fa00116e6173243473e292370aa7429b693bb8527192fe171dc25e642ddac35c42da0e0f4f15f3f45246a720e396433f58d0a5efcc3bd2c41ea4c1a9e7f91fdd03f88a068eb77e2f812d622596e51c1cb7ba59e9ea9984c9d692a615ec3b75705a6c386a09afddeaad8be58b213f1c59683b25729c4224d94d1d578954331416a8320113a80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8718080a056ac2bf3a1e15140f45dd68a0c792972607b081b6632744d199ad0fea6737c85808080808080a09eb6b46c969d6d257a7c50b402407583a062452a9a734f91aa091a1d4af8d3d380808080a043af85dfbcc3c4ecb1143422355e6716a57c711b40c0a972c3798eb7954cf0d48080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a02069ad09c79cd38934b567412ffa82bf024fcb938a8b7ec112fb038de5047bb2a1a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec()).unwrap(),
        ];
        let message_id = 2u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001").to_vec()).unwrap();

        let proof = EthereumProof::<AcurastAccountId, AccountId32> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}

#[test]
fn test_send_finalize_job_message() {
    let mut test = new_test_ext();

    test.execute_with(|| {
        // pretend given message seq_id was just before test message 75 arrives
        let seq_id_before = 2;
        <crate::MessageSequenceId::<Test, EthereumInstance>>::set(seq_id_before);

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

        let ethereum_contract = StateOwner::try_from(hex!("57e796ff645719FC98b9Be0b3731F1605Ed98c5e").to_vec()).unwrap();
        assert_ok!(EthereumHyperdrive::update_target_chain_owner(
            RuntimeOrigin::root().into(),
            ethereum_contract.clone()
        ));

        assert_eq!(EthereumHyperdrive::current_target_chain_owner(), ethereum_contract);

        assert_ok!(EthereumHyperdrive::update_state_transmitters(
            RuntimeOrigin::root().into(),
            StateTransmitterUpdates::<Test>::try_from(actions).unwrap()
        ));
        System::set_block_number(10);

        let snapshot_root_1 = H256(hex!(
            "165f651aca44dc76ac642127d4a904b2270b22459c13b1bd5a360ea25f314f1d"
        ));
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                snapshot_root_1
            )
        );
        assert_ok!(
            EthereumHyperdrive::submit_state_merkle_root(
                RuntimeOrigin::signed(bob_account_id()),
                1,
                snapshot_root_1
            )
        );

        assert_eq!(EthereumHyperdrive::validate_state_merkle_root(1, snapshot_root_1), true);

        let account_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a0dded1ea1ded6da9ae53a491896a34344c885a87123a0fc3406403c40dada8fe1a062fad4c414d0968b0d4b61b06abe4a12785c05dc94f2052087a24603377a1127a0762dbe56774d5c2f74f3bc62eebbff2c1ef8d622e044d763ef46a9c6e05b0895a024e16ad0a70f8e2a373cd20fbcb1beefd236930c9e80972676ee75e71376f748a05c0b8e998808ff95805f6e6d114aca2bdbc3c8a05cac100caa83453a80fc551da084cc118be70d76f1d7d45e731adb32a5ea98194f1e55e12e8a81c1265bb35804a0cfe0d0d5a659bf87435d9175311ce312a99fde37a714844fe8a86b6671485b29a014d0b6e6bd843f8f5652a691b07e8bd57d8daefebf68518dfa1ca2c27371c955a0e59a89693faa726341f6eac51e893e81bcdf3983788bfcf88cd35e85f2692072a074ad7d620707530ae12cef2d83f28cfa6b381176f2f17dda157f3d94194c3edaa060e8042b25a70cbdb5b7bf7acd57e96f389f8c5ca61568a3d104bb6e89410397a00be1c2a477f8eac3a4ba620b959b4e9702f919ef860337420a3dce5be814158fa091dff9cd84917e1dad5c65c824aa549be1e2b537eaa84adebbf0c7a34c0516b8a09db42f5adcc060c2fb368c2b5b5f2df2bd432018e76c5497305cf9ab040ece2fa0827453674aa8a0721788852fc341b097c451b87949198314a6df7bf7640c34bea0926595591284d182861c26ea9e71e86c7b9f8ee3b6859ee29113ebe08026916b80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ee8f768253ed6c37765832d64884a5e092e41cbfdaf0fb8f0932211ce0d568c4a0ce495a7fa0836aaeab91fbbdc7dea5ce1aa96b857fd1a8e39f86f254010f2973a0491069c53daf75d1997636cf24365e3df2fb7b0c1f473512f4015bccd7c0c495a0bc5e1a2ff02cee76141ebbde7e11ef1cde35ea2870dfb9c86c3686a1149359d3a0981b41642f7de62e3b86aacbaf87550fd5e6cd25fa74184b83547adf23a22fc5a0fddadd87475a00c34bb9d6f0247c6f2e1aa35a9ae3f704e053a8a5c2ce5c3913a073f979076c82f9b273fdb1490eed2a42118a22657e9e4aff5531b9434b601bf1a034f4c189b0d5b87be223f8d9ded6ea924b183ee5cf6559406de3c8c0a748df0fa0ad24cc8adf2c208cd3b4a618b16bbf9ad14caacd2b07fbc90bb62df6be85496ea0ea35325f678defc5ea53d77a23a293fc68c2d0b39fb1231466cd6c234da540b8a00ba70a69e36fa88db1d0eaa49c5b55e898560a8f7eefe11b6784020be2dbd8f4a0b3caf50d4308581d338e3ea7c22525d82188aebbee020b632587460edeae4af8a02272da0b5d0a15df06e7afbcb3c72d46f014ff8ffc767fed67bd913f7726681da0b7e7835994dcd6ff3b2acb150d385946911e12cff2deebce7b2c1d330443812fa0eaf2a4492c71a0c4f472d0def98e87407968800bef137205774089c5f516fcd4a093473cfe514cb3d370ab4be46ac4e5f4d7789246018164ce27b0aa67f389665e80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a03467744e52388cea47422c20dd914ba06b7774b831b6495bb4698c15d389148ba088525a844b5a5d4d8ef78f2a3778ed158134c67bf398c7d3bdf75eaeecaea572a05eb80f9c3226203a10218dc0fae5ae105405d88965dae3f54e8182a78cce661ca0fce6fb480556411a6154d06549bb13faead17c58ad5a2f007dd15c27c8b71ab9a039ce7123fca637f72e2453bf5fb6d41d339c9ed89d952528b2834864b3b95380a049082b9365e9191eae486a26eb45c46b86c684f1e681262ab2901e8966f00e9da0d4033860fd491626ee7b19e5a139740adb20b20681a4caa3476d192bbe0fb394a0c93505a507ac2a8c02679f893abd01c472a1d4f55da00fdc761d8b05ec3af045a0974c124e54de2d0d35da4e8f96e0e79594f1fb8c89eb9988dcce906ba03000a4a0d2b3c203d5ec947f1a8fb4d1bb9d0c988543f26aabbc41348e539a1611b2f1dda08829379b6999f03962ed414b65a0a23eb969525a0ad183182c32b116cc889be1a0721b85e9bb81dba6362be176d5a335ed706ba83d0e977ef7621d72060d84e28fa0174f7dc9393883bcb4439c7af8b090e45edd7e812908bd8ff47903f371471fc3a0713ab60afd75ae70440f07eac886187729e27b06f760f056e81cf2ea72b766f0a0197b0c5479eb956a72b7b52e5b4a6c2617afb71d7437559f208850d3b2fac728a0d8a6b21e99114d2bcf3ccc468bd6bfd3c6cef707c930b709294e90b537d20ead80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0ff0a1740e1d2306fe57d45a2a282ac854ba247fef3d5b2bc22e83ee38a381578a0d2e14547273fc537625a75c302f56ee66cebc4768a150cfe58001566d7aa2d7ca010ee81a990481ea9d551664be71e8d9151dc05c2e11f9580f744b4c2a797ebfca037d54c4a3a414ce5b1dd5b05f54c3f9eb188a83f211d10e9856f3aee9fbe6c13a0afd0cb95be49860f9374c39afdb6bfff7bb920c1fa2b43713a62496198756b97a0316aa0c4bab2b1863dbfa2ec6ebadb68177d2af31c9801320c7ddfd26837a754a09a22b6f4e8b3bc901cdf1904c52a74f60e467521b71571c8919291e38ce646d4a09a67f363de88b930c48910acfe09799e88ad844ddbfe13cd644e755e99007694a0d0ebf268b32959711320592556273d90a191832efb32dad8313e454ac89b1203a00fbd113c8f88fa023ef525f1e963ba11cb31926bfd8e78e4a3d3a494cf011da6a086e5af77f6983dff46261e4ae6b04b26af984341fdb8820371b209c07baa8b31a007905fa7cfd6e1e359772711455b981134fc3a61748229d8a8fd0e38e7b4345aa077948f4f9040d14cfbfb4e6a4d998665c2284e6e613f5d6cd66e5b34e58fd7f9a03b86f28bc11cee028ed3b69fc2561ccb55b8563225537db8fce33cd7346b52cca015ca6665e74023a3d81068daf162f1dcd99880d0d0b9c4c08bc7a9011bcbe31ea051df22083c50d8216e7ae63ff8386c8e013ebfe146ac088cc068e8a98fbf5e2080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0f8e2caf4bfbd2387b011ae9b4e0434c4ef6c17fb7104bd3851ca9aa14c4c7be3a0b206684c96b45284dc843db0ef51474ac844697367d1e5f7a4b09b1d5b88b158a075a3fa28669ab58184a2e5bcd13eb55bf4751cfb4860c8074ff70442494d2229a019045885703bff231764c236c2dee89caee45ada51cebc30b8c2efe81a9f3782a0f453685df20ca90e4d1d642f1473139fff91f4436902baae4e704ad26976fc7ea05001ac402c71ec49abe1c679a5c6fd4fa0e55f49c597bec9d76de9cbf3ff5c08a013a1e86fe06edd27811e5129ea1730110de04bbd34e9a7bee5b717ec0023b78fa0a5e1c975d1f3ccb82efb8fbb767d9ce88851e034d9744837d59ca93b3f6c685ba07a0663e7103382619141edebc678ad5dedadb7aaf85c58289601fd583a35812aa0a15e7842d62674143b0e47ec04e5112e67d45ae9f89abdf73a421f0693a782e7a027b21a1eb9d192bfc7c24029690a3ccbe16771b82830dbe86ebf8e1be58aa8a0a0331021dc66a180e92ac2e1612db10bef58e6b79e86c1823c2dd5a906240a0930a0cb5bfa5dd659f5b0fb20ef8cc9471fa0abf204109e5486320f58d0d930c3c77ca0a0f05d5b9e5364abc8803976ba186997edafbac57fd64fa92a653010f62415b5a0001b4772a49facffbfd536e0e2dd12140dd71adcdf5a7e6d489e329b061b2585a0a029809ef5d8d359bd8f7fecf169578d49c6010a64b32c8649bd46e1bbfa65ea80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90151a06801d40737cad61186b939eb4104c5502273d26c535f7a37c34a63abf6e4dfe980a0b31f02a6b752f8eba131c4ec775ce0444ad2857792d19ccf587726dce76a273280a0027f124239b915e70ef1a986bfe2dfac456fd44237382307eb754ecd44e3821280a036ae5c8e0cc6004f5d1d4c66c236f7930ba2f64cbae6d9f0363876e47a01445aa0afd1c866b64cf9611d0c3989dad1a383fc035cb72c0bcd0882444d77ecbe0cf18080a0fa1915f63b7faf7d5b2a76aaeeaa9a61a7ed451c1f4718e8bf3e5af77581eac8a0e91198c91dcacf9d20eaadff1f6532676f2e377287e68c88b043d2aefe8e9119a096f9d03774a23f2570ffb8cc4298ffc267f4364bc746069975f21c45726fc71a80a00442927e748e876312dea52b28dfccfcf0b18b3daa10e62a317c40e858dbb201a0e88950e7888948366eb14c601bbc63a1ca546fba0aedfa5a38df928e5ee7982f80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f85180808080808080a084e54b0b5c476fbaae3840e18ea5f1b54d2cbfd4b5d11c3945113253b2966836808080808080a0e71dfd63f0c4ffd6568e8230d4ab61ff8a344ee88295aa8da2c6a5eea96960288080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d3eba0fa7e2848bcd30e1bf958707d2a02f8f03ae438d6622a21b562c7fb846f8440180a0ab4f5e5ac89f9bed9eab40a5b02763168e73ca32c5dd9f5ced76ae92815e42e1a03c789e7c0b32cfb991ed499ea05ca68b7ae8e89f5ed2bbf04deb5878c4018f68").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90211a04d6190501267fd3b5a9d1223d003b4bf5ae76063514a1033389b366426814821a070bf5d648f0dc17e554de24b7f1b7c803652abc65f216feaca4e1173eb334b10a079e72bd9aac24a2713cd23c2e3e838573073440d39c696b37b9a00343c590433a0d0dfa426aabf2e1cc49d1c4e990de5b4acf44d45337c2ffbd7403e9f3e76dd2ca0e03f9e0f2f37b58194656eda176816309348200438b94ed468f49f9b52539003a0fbb5f5c46cfbbec7dfaca1ab5bb383bb53f1f4b92ac955b7577c79877fb4a757a09eabfc1cafe877b5a9c85323492bf327c6be4ba87c0e575312f8a3ac0dd11707a09fe41785607793ac153fc5fba450a0ce95778bf772d42cb6514f152624622ab8a02018a6ee674122dffe10c2c7bfc53d032db313f05ea83fb254735f419611a37ba0d7a47dc607220e44942fa5f4c0475ae561777f2b13481d019dc69d474aab9f35a01d93a510fea6b836e1a07a7ce7dcaf42c46f60d2b136e0183e43d833069c36afa0a0d044a21761c2fd8ef694e0e06ccfb0d27f51d99049727fd06237b43835ad0fa00116e6173243473e292370aa7429b693bb8527192fe171dc25e642ddac35c42da0e0f4f15f3f45246a720e396433f58d0a5efcc3bd2c41ea4c1a9e7f91fdd03f88a068eb77e2f812d622596e51c1cb7ba59e9ea9984c9d692a615ec3b75705a6c386a09afddeaad8be58b213f1c59683b25729c4224d94d1d578954331416a8320113a80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f87180808080a07b194c6bcb312999af4354bb70521f21eb1903e20ffe40e83c2cc5cb4c3b4d90808080a06fdd784c6da39cf6eaf411b98d78558b500c5321600cd16cce1f444d520f1ccd80a0b1ddd7cff7e3bd00798e7b6af29cfa2b075f30d9674bc73c3d01528029605fb6808080808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a020a6d7be7cc8683d642e1b02ca73839c8d0ffcdbc19dc8f945e8011ef97ac1aca1a0a9b4d7f21621105cf248706c7b901f4e4d74cd7513562b306f3206d3351f4ddb").to_vec()).unwrap(),
        ];
        let message_id = 3u128;
        let value = EthereumProofValue::try_from(hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000918efef09c0ef0fdf488f1306466cedd9e741b6b000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").to_vec()).unwrap();

        let proof = EthereumProof::<AcurastAccountId, AccountId32> {
            account_proof: account_proof.clone(),
            storage_proof: storage_proof.clone(),
            message_id,
            value,
            marker: PhantomData::default()
        };

        assert_ok!(
            EthereumHyperdrive::submit_message(
                RuntimeOrigin::signed(alice_account_id()),
                1,
                proof
            )
        );

        // seq_id was incremented despite payload parsing failed
        assert_eq!(EthereumHyperdrive::message_seq_id(), seq_id_before + 1);

        assert_eq!(
            events()[5],
            RuntimeEvent::EthereumHyperdrive(crate::Event::MessageProcessed(ProcessMessageResult::ActionSuccess)),
        );
    });
}
