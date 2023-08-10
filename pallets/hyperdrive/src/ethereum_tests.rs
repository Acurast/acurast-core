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
fn test_send_message() {
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

        let ethereum_contract = StateOwner::try_from(hex!("7daEe65f9aA9028926deCE01cCfD0bD1B9320522").to_vec()).unwrap();
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
            "51ef75e6b8d780882fc5b708d06cd7938b0aad95ecbd12820af8055c2651fad9"
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
            EthereumProofItem::try_from(hex!("f90211a08eaf6ebb8e1bb1a8eea303776055f7b2b1c74ef3b630ccb524416c7ab47c73cda07eeb37b7f8ea76566011d359cd72d2749eb83ebca08f5e541b18d8c99709c9b9a0bf4c24b0b032bae856fba0ae1edb5e363ad50d6b2e597af00d9150157afc7e25a03f7eb09872d36228948eb72a2cb5dc884d9bad5541812b038d11bd286312a456a04325c25ed1c87271825ea0c5617ce9ffc53af6fd0570a55e5ba1dfcf2e96792da0de2b583075dfed409c5a65d5890e6929b3e3c9a5b9e1c61cda1b70d84b7ea220a014534d7c9cb3bb695c943765f97355f041009300bbc55cb362f7604ece7de2d8a0e978c291fe15c76667eeb8a7589ace5f38718c6d1569875631443d30384bb512a05b187621d8643a1a344542dce2b27fb7ac7b50f9247b4d07c7489f4f247d1648a0d37e3dccab798819fa3eb97dffe50871fcdc725c4ffc3f09b83fae57fdb85f1aa0a03beb99e39f9157e4e0476a2eda6e68e29df1245488b37ae2c698112cfe4a7ea0a811d114469306486627734d8a3bd5207cf0054addfe631a804c9d5c607a912ea0e7d5a0ffb9da633f4d92421cf37da23776038d16c6cec05b77698c2f6c53a5a3a0932b0f145156fdf12fa3bf1574f6ad6065dd2e5ab652709e6ca8f47e3adbe5efa0e37025849eea1505c860a77507fdf800d7be4dea7192162bd759fa869fd16458a0506336426f3481ffb0319add732c895374610667f354b9243c0f0b19af269d8280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a038207fdc3b7521c09705afc7391e3c6adb4766efa4342dda718325fd8524c0e9a02b109f7b450d63b57afa25e3d2bd121f458c1bbd7d42cbb0624889e5d9fb6711a055e55ec69abfa867d03f1e554d5a450d1aa1f3e0257006a236db2e501038910ca09cd09cd71602059dc3b44afe5d1a13d062a9f718bca127e0a1d033d3c996c9f2a08816fa1848e847f2dfc2da95f84715824f9771b1b991ecd8f65bde390fde11afa0e0629cfa3879257b862930113009efd1eccc9b7a019061bf88ad38f0b41683a4a0acd21f084dd347e7a2f1baf72d8d3828a303d7187ee5d060728ae63a40334ceea07180f4b07ab8d296ca85d7e12a256d09d01121ac19a7d66805fc6ce4e1d586fda004ad949069ae83c0bd4b677908784b01baa7072c81d633ba9a23016304d180b7a0d5aa6ad4cfa51024598ec85e1d987fbfb5378e74166d24b7300b41346110d1a3a07f0217db68c4d5736045b8fd11545a6a384c266764ce7ce2b473f0239f29c81fa0510a76f022e865d0043fe111fa1343e9980871200f2426619f9c203c67b60e61a0800685ad27aa943dd01f64ad9acbec78b3843c90648c6246a5cb2142c8b14d7ca0e40e90d3a5c39d84323e78f830b4912c99343a18277e836848ca28a2a7f1a604a00f7efb94967018bc523fb61c44f7aabd4c921aba3b6a5f6ba7d1aa4473ec0470a0876d4c60081251092ec22db126aefd4f5d3676e5f8d52c3bcfb47f20ba71539180").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a01418812492def8e5abac29cbe106579c74b5e08e40d3f44938443f22e0f20b34a0a704561ff1e2b1de9cc83e1100aaecdb8fb03c3af85f5de3a0197f04286225e2a024a84b3ea19574d04f33331eeeae2da712cdcefd3596036182e26b9c64be211ba00f4643285ccb0b5bff57e9dd852efb7ac99e1ac9c7cc662a12b0d12444634c6ba0db858ea10189efa17727d68a4dd538b5de912a42366488a2e33e7549cd05e164a01bca9d32ed8d26148bcb3d88ea358bd08b21b195a0d0ca2af125ace493364f7da00c4ee20a2401f125c50e465dfaf532a72683ad7f260d83cef19b1a3bd94e0325a00973b86408fdf47b9532b5ca1a8a8c4205f4afad6322019a17d72930b8fd8f90a075952915c053ff8e3f26e1d9e18c26b003fffec7ae3cf681f2a26c2f19e85bf9a06fbcc396a4630ca2f0b7ed941b2bfb2d0432cf8f5afae27e35a7f3b5553c3502a09b64ca83254c32717f35b81edda1e80132525b57a40bfe3108e895045fa03a78a0374a13a09dc40edb9e9a125159ea3ee8d26b57aade9697e572181c912da96737a03e7b3ad2c599449eec5daac44a8e2debff0f6054752f0f40d9219cccb7b33787a0a5ba91f16ca2e0a24379ad27e5c73bc0870ccc3d012fcbf52120327276d6bf7aa078e9aa6a6fe08641e7e34d4cac42e118dd958c5dd048508e3a5e22238e401c96a0807b0990f46405dcdb7e1ceac0cbb515b8783ba8269893e6bb5451d593e0a83d80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0a6b11c1c10c1946db1c875208c0ead2a94edbb9aa42f8dbc7a1bdff82f587b1fa01511731577dceb39b7c7c096fc223d26accaadf01e4c1b9d4fde9bb0c58cd16ca0e10af52705710a7b28c56c89fe4995542c31aa26773b02880ab7f9c03bde7860a0fe4d40551600d653b489f779e6fceff2ea991c1fc84ebef7573a5d606a4cf29da0b451d89fd7781bbbb5fa4d9dcf76962977feaaa8f196a5026ebb29198388fcfaa0c2cb060012f46d8b70b687a28c8290ff1c8f20d475ffb911ea198e0ce77da02ea03f4d9ce25e3b0149eeb6e344f192c00cf32a9e81bd7cffabbe48630f2abb26b2a00a867a4827ec8d6b4511710cc924bc301b642afd4ad4f2d4000433138c80e0a1a035440a47a1afc0cfa000b5d66bd9d62a1dcfc086b65bcec79ee7564c0de22ef4a05786cd06b288a8d1dfe7aaf1a5d9af860291d630e9e610120321b33b099cf6b4a0720d9b1ee6d11ab6bcc63f3b7eeb7f3e7fab265f9637f0defee79851bdee89d0a0cc8301ccc775d7ae2eb0323922dbdc64ef7d113475c2b7034705a4e89e1ed565a0b757a57549571e3e99be553dc18450438099ea98726eaeb663b33de4cd905ed2a0934a707348fe85d0cdea7d192f7ee9833e011035bc270ae76582980c4ea73e4aa09e29016e4d6688acf05cc1076f92b6cd73279332aec57c3513d31283ebb63e73a0d60947b8a989b3416c8ee56827d32a37a654b684fe4d193133504edf4b04f03c80").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90211a0a871491e5b929da4f3bc6fb9280062d5f76387c9ac0418c917a07e3203bf634ca0206a35ee85ed6a4c23195503cbfae6e32cefd72fde577e3894be442403c8bf64a06909320508992188cd0f2b5f417b5a1e4122b9e29765c41aa7fb90da1c70fd0aa0ab7d369b0f3ffe644d9f65a94b01ca21e655c4bcd06cfb332f7609a26955f98ca04f0a9f060d209a30be25adda85f648a38601ec0df850f4f9ea3157baf2451b2ea0d9e74ba1106ac6d054d5c40ca76d1d798ef8aa5efc19788de41bde2bddd18fb9a02213c86409a5b8b81c46ca9be306ebd375609676ba1487c338a9a7b4ade123a1a086c95c068df32e311982d5d5bdb4d2663ea756b0b4f486f987851c4797f5d81fa041e9304393573784e1158c36b3538b48647123a1b4696fa31fec146ed3230af7a0a4284f9ab2d0e99b59e22ca87297efdef7c4adcd177f01992588e3b17b371f1aa0aa63c261c542ceab16daa41935809ff3c2cdb360a2a2ed4b7feadeca8347ea2ea03b7018988b228fb8c11136112ab307482188f1e2bf7217706db87cb9720810e8a03f67a8062ea47c166a77b827b778201af6c2bb365965f322b024127d146776b0a06986529d536a4511c7affe6c3ea557a1f36d6effae62fc7c9c557276d3b9787ca098d1b970a680d7595f92224fbba919039c7753c8a0663e035998817d7e49cf3ba0f197618bc578a56aa1ad0e2f92e8e830e30ffde40db3419c85184fb19ca197f780").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f90131a0bcb355b48e34a1810bf16318e5fda0b3b4d13b0917d201ccf5cc4f411beabb92a02b5988931b7bdcde64b31059bfa56cafbff5e1396055038a05e3529027f6ac6a8080a0db5ad8d712315cc101403ea2eec4a1b6fd53a551659004846864a591e7e70fd08080a050386e923f1d64491e2c85db30c92bca10b5c1fbdc9c3289aa01469c443f93b7a0f1b377468ffbb0c483d90f15823dd5a3ccff71f0466e4b50298a5cc251835b97a09f0b569a7045b73d92262955b755245e350e5c94f0eb1c8f7d6e58d4425d4caa8080a0b24b79b089a47fc1674caa805f9445abf7bb1a7677fac085acacf7fa83fde77680a0c24c2cfa8bd482d61fc2a8b1bb4b59816ef52001b48158ed6f63bda7f408253ba0ab984464dc8740e51ed457c752b65a8ed9f574468571543d5a7da29dd12a49f280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8518080808080808080a08dc559bf5bd4548b32fcc32181d86612ba8b11f1bba9261c2ef7fe2f34557bef80a01a280c5a0987cc7c2cb878c376d6a2e27e22925c372f2ad15b9ae597a5ca146c808080808080").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8669d380c5e978f3f36e8b029833fd159cba829a54e8d55e8c4e5f28b4831a1b846f8440180a0c53cbaddd072fc5094f0e0986a1baff9ed3d6dbe4133eb4e7764dd9e93f9ec9da04d9be648c5bf39973670d9f8b481d5d0b971e6a2db2deccc6b98cde21c5dd83e").to_vec()).unwrap(),
        ];
        let storage_proof: EthereumProofItems = bounded_vec![
            EthereumProofItem::try_from(hex!("f90131a0fe1cec69138a035b27919cba7d03d2f3b5867e183fc5928af3bc0b0f85b562a880a0e759fad30e475a8a7de20efb084aeaad48864ef0c5eb678f0133226a4489d5f8a0da9cbdd2154724e704491b792e162e096df39e9f51363b9b950933a61186820280a0de572a50aef9d550512795e67eaf06acda25ada12d45e5944fba2cb429641f5480a05abb50d3ee32dffe73e3a7f9f354bffe92e4971bf45b527d046208a6818120f980a0ca5985306e251400a05df43a16a3391bca6cf1e5a39acfde6f619c8ea03e3fbc8080a03783bc2fd4d98095264ccacf2098c92a04e317f93a82d87a713d645e0743ef8a80a0bb7fbc81f9cb125fa6229c00a3b6442d31316510f0a9054827bd2317fc95ac9ba0b60d522b76ccaef75c1d5d2faf67a3904ea0aadfe459950661a60e2111e94ca680").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f8518080808080808080a0ff1d82682091977c3bd249fd5840706e2c8f487add0b1ae09d430e80d9aeb8f9808080808080a066ba505307e91ddbb884cf21cfffd24941ca533e0b9384a68144039ab7fc57a280").to_vec()).unwrap(),
            EthereumProofItem::try_from(hex!("f843a0202ead72d53401d823f4de3290714b95c588de2c574133f57728a2d3d3763d3aa1a0f03ee4236f341d60bc114bdc519db37d120d1d98b8d3f12b9b6a65c2aa99b01d").to_vec()).unwrap(),
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
