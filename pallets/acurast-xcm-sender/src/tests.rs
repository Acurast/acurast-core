use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

decl_test_parachain! {
    pub struct AcurastParachain {
        Runtime = crate::mock::sender_parachain::Runtime,
        XcmpMessageHandler = crate::mock::runtime::sender_parachain::MsgQueue,
        DmpMessageHandler = crate::mock::runtime::sender_parachain::MsgQueue,
        new_ext = acurast_parachain_ext(2001),
    }
}

decl_test_parachain! {
    pub struct OtherParachain {
        Runtime = crate::mock::receiver_parachain::Runtime,
        XcmpMessageHandler = crate::mock::runtime::receiver_parachain::MsgQueue,
        DmpMessageHandler = crate::mock::runtime::receiver_parachain::MsgQueue,
        new_ext = other_parachain_ext(2000),
    }
}

decl_test_relay_chain! {
    pub struct Relay {
        Runtime = crate::mock::runtime::relay_chain::Runtime,
        XcmConfig = crate::mock::runtime::relay_chain::XcmConfig,
        new_ext = relay_ext(),
    }
}

decl_test_network! {
    pub struct Network {
        relay_chain = Relay,
        parachains = vec![
            (2000, OtherParachain),
            (2001, AcurastParachain),
        ],
    }
}

pub fn acurast_parachain_ext(para_id: u32) -> sp_io::TestExternalities {
    use crate::mock::runtime::sender_parachain::{MsgQueue, Runtime, System};

    let storage = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn other_parachain_ext(para_id: u32) -> sp_io::TestExternalities {
    use crate::mock::runtime::receiver_parachain::{MsgQueue, Runtime, System};

    let storage = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
    use crate::mock::runtime::relay_chain::{Runtime, System};

    let storage = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[cfg(test)]
mod proxy_calls {
    use super::*;
    use frame_support::assert_ok;
    use xcm_simulator::{Junction, TestExt};

    #[test]
    fn fulfill() {
        Network::reset();

        let bob = frame_support::sp_runtime::AccountId32::new([0u8; 32]);

        AcurastParachain::execute_with(|| {
            use crate::mock::runtime::sender_parachain::AcurastSender;

            let payload = [0u8; 10];

            assert_ok!(AcurastSender::send(
                bob,
                (1, Junction::Parachain(2000), Junction::PalletInstance(130)).into(),
                payload.to_vec(),
                None,
            ));
        });

        OtherParachain::execute_with(|| {
            use crate::mock::runtime::receiver_parachain::{RuntimeEvent, System};
            use pallet_acurast_receiver::Event::FulfillReceived;

            let events = System::events();

            events.iter().for_each(|ev| println!("{:#?}", ev));

            // Check emitted events
            assert!(events.iter().any(|event| matches!(
                &event.event,
                RuntimeEvent::AcurastReceiver(FulfillReceived(..))
            )));
        });
    }
}
