// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use crate::mock::*;

use polkadot_parachain::primitives::Id as ParaId;
use sp_runtime::traits::AccountIdConversion;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

pub const ALICE: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([0u8; 32]);
pub const INITIAL_BALANCE: u128 = 1_000_000_000;

decl_test_parachain! {
    pub struct AcurastParachain {
        Runtime = acurast_runtime::Runtime,
        XcmpMessageHandler = acurast_runtime::MsgQueue,
        DmpMessageHandler = acurast_runtime::MsgQueue,
        new_ext = acurast_ext(2000),
    }
}

decl_test_parachain! {
    pub struct CumulusParachain {
        Runtime = proxy_runtime::Runtime,
        XcmpMessageHandler = proxy_runtime::MsgQueue,
        DmpMessageHandler = proxy_runtime::MsgQueue,
        new_ext = proxy_ext(2001),
    }
}

decl_test_relay_chain! {
    pub struct Relay {
        Runtime = relay_chain::Runtime,
        XcmConfig = relay_chain::XcmConfig,
        new_ext = relay_ext(),
    }
}

decl_test_network! {
    pub struct Network {
        relay_chain = Relay,
        parachains = vec![
            (2000, AcurastParachain),
            (2001, CumulusParachain),
        ],
    }
}

pub fn para_account_id(id: u32) -> relay_chain::AccountId {
    ParaId::from(id).into_account_truncating()
}

pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
    use acurast_runtime::{MsgQueue, Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(ALICE, INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn proxy_ext(para_id: u32) -> sp_io::TestExternalities {
    use proxy_runtime::{MsgQueue, Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(ALICE, INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
    use relay_chain::{Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, INITIAL_BALANCE),
            (para_account_id(2000), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub type RelayChainPalletXcm = pallet_xcm::Pallet<relay_chain::Runtime>;
pub type AcurastPalletXcm = pallet_xcm::Pallet<acurast_runtime::Runtime>;

#[cfg(test)]
mod network_tests {
    use super::*;

    use codec::Encode;
    use frame_support::assert_ok;
    use xcm::latest::prelude::*;
    use xcm_simulator::TestExt;

    // Helper function for forming buy execution message
    fn buy_execution<C>(fees: impl Into<MultiAsset>) -> Instruction<C> {
        BuyExecution {
            fees: fees.into(),
            weight_limit: Unlimited,
        }
    }

    #[test]
    fn dmp() {
        Network::reset();

        let remark = acurast_runtime::Call::System(
            frame_system::Call::<acurast_runtime::Runtime>::remark_with_event {
                remark: vec![1, 2, 3],
            },
        );
        Relay::execute_with(|| {
            assert_ok!(RelayChainPalletXcm::send_xcm(
                Here,
                Parachain(2000),
                Xcm(vec![Transact {
                    origin_type: OriginKind::SovereignAccount,
                    require_weight_at_most: INITIAL_BALANCE as u64,
                    call: remark.encode().into(),
                }]),
            ));
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::{Event, System};
            assert!(System::events()
                .iter()
                .any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked { .. }))));
        });
    }

    #[test]
    fn ump() {
        Network::reset();

        let remark = relay_chain::Call::System(
            frame_system::Call::<relay_chain::Runtime>::remark_with_event {
                remark: vec![1, 2, 3],
            },
        );
        AcurastParachain::execute_with(|| {
            assert_ok!(AcurastPalletXcm::send_xcm(
                Here,
                Parent,
                Xcm(vec![Transact {
                    origin_type: OriginKind::SovereignAccount,
                    require_weight_at_most: INITIAL_BALANCE as u64,
                    call: remark.encode().into(),
                }]),
            ));
        });

        Relay::execute_with(|| {
            use relay_chain::{Event, System};
            assert!(System::events()
                .iter()
                .any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked { .. }))));
        });
    }

    #[test]
    fn xcmp() {
        Network::reset();

        let remark = proxy_runtime::Call::System(
            frame_system::Call::<proxy_runtime::Runtime>::remark_with_event {
                remark: vec![1, 2, 3],
            },
        );

        AcurastParachain::execute_with(|| {
            assert_ok!(AcurastPalletXcm::send_xcm(
                Here,
                (Parent, Parachain(2001)),
                Xcm(vec![Transact {
                    origin_type: OriginKind::SovereignAccount,
                    require_weight_at_most: INITIAL_BALANCE as u64,
                    call: remark.encode().into(),
                }]),
            ));
        });

        CumulusParachain::execute_with(|| {
            use proxy_runtime::{Event, System};
            assert!(System::events()
                .iter()
                .any(|r| matches!(r.event, Event::System(frame_system::Event::Remarked { .. }))));
        });
    }

    #[test]
    fn reserve_transfer() {
        Network::reset();

        let withdraw_amount = 123;

        Relay::execute_with(|| {
            assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
                relay_chain::Origin::signed(ALICE),
                Box::new(X1(Parachain(2000)).into().into()),
                Box::new(
                    X1(AccountId32 {
                        network: Any,
                        id: ALICE.into()
                    })
                    .into()
                    .into()
                ),
                Box::new((Here, withdraw_amount).into()),
                0,
            ));
            assert_eq!(
                relay_chain::Balances::free_balance(&para_account_id(2000)),
                INITIAL_BALANCE + withdraw_amount
            );
        });

        AcurastParachain::execute_with(|| {
            // free execution, full amount received
            assert_eq!(
                pallet_balances::Pallet::<acurast_runtime::Runtime>::free_balance(&ALICE),
                INITIAL_BALANCE + withdraw_amount
            );
        });
    }

    /// Scenario:
    /// A parachain transfers funds on the relay chain to another parachain account.
    ///
    /// Asserts that the parachain accounts are updated as expected.
    #[test]
    fn withdraw_and_deposit() {
        Network::reset();

        let send_amount = 10;

        AcurastParachain::execute_with(|| {
            let message = Xcm(vec![
                WithdrawAsset((Here, send_amount).into()),
                buy_execution((Here, send_amount)),
                DepositAsset {
                    assets: All.into(),
                    max_assets: 1,
                    beneficiary: Parachain(2001).into(),
                },
            ]);
            // Send withdraw and deposit
            assert_ok!(AcurastPalletXcm::send_xcm(Here, Parent, message.clone()));
        });

        Relay::execute_with(|| {
            assert_eq!(
                relay_chain::Balances::free_balance(para_account_id(2000)),
                INITIAL_BALANCE - send_amount
            );
            assert_eq!(
                relay_chain::Balances::free_balance(para_account_id(2001)),
                send_amount
            );
        });
    }

    /// Scenario:
    /// A parachain wants to be notified that a transfer worked correctly.
    /// It sends a `QueryHolding` after the deposit to get notified on success.
    ///
    /// Asserts that the balances are updated correctly and the expected XCM is sent.
    #[test]
    fn query_holding() {
        Network::reset();

        let send_amount = 10;
        let query_id_set = 1234;

        // Send a message which fully succeeds on the relay chain
        AcurastParachain::execute_with(|| {
            let message = Xcm(vec![
                WithdrawAsset((Here, send_amount).into()),
                buy_execution((Here, send_amount)),
                DepositAsset {
                    assets: All.into(),
                    max_assets: 1,
                    beneficiary: Parachain(2001).into(),
                },
                QueryHolding {
                    query_id: query_id_set,
                    dest: Parachain(2000).into(),
                    assets: All.into(),
                    max_response_weight: 1_000_000_000,
                },
            ]);
            // Send withdraw and deposit with query holding
            assert_ok!(AcurastPalletXcm::send_xcm(Here, Parent, message.clone(),));
        });

        // Check that transfer was executed
        Relay::execute_with(|| {
            // Withdraw executed
            assert_eq!(
                relay_chain::Balances::free_balance(para_account_id(2000)),
                INITIAL_BALANCE - send_amount
            );
            // Deposit executed
            assert_eq!(
                relay_chain::Balances::free_balance(para_account_id(2001)),
                send_amount
            );
        });

        // Check that QueryResponse message was received
        AcurastParachain::execute_with(|| {
            assert_eq!(
                acurast_runtime::MsgQueue::received_dmp(),
                vec![Xcm(vec![QueryResponse {
                    query_id: query_id_set,
                    response: Response::Assets(MultiAssets::new()),
                    max_weight: 1_000_000_000,
                }])],
            );
        });
    }
}

#[cfg(test)]
mod proxy_calls {
    use super::*;
    use codec::{Decode, Encode};
    use frame_support::dispatch::TypeInfo;
    use frame_support::{assert_ok, traits::Currency, RuntimeDebug};
    use sp_runtime::traits::AccountIdConversion;
    use xcm::latest::prelude::*;
    use xcm_simulator::TestExt;
    // use xcm::v2::{MultiLocation};
    use frame_support::traits::PalletInfoAccess;
    use pallet_acurast::{
        AllowedSourcesUpdate, AttestationChain, Fulfillment, ListUpdateOperation,
    };
    // use pallet_acurast::attestation::{CertificateChainInput, CertificateInput, CertificateId};
    use frame_support::dispatch::Dispatchable;
    use hex_literal::hex;
    use pallet_acurast::Event::ReceivedFulfillment;
    use polkadot_parachain::primitives::Sibling;

    const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

    #[test]
    fn register() {
        Network::reset();
        use pallet_acurast::{JobRegistration, Script};

        CumulusParachain::execute_with(|| {
            use crate::pallet::Call::register;
            use proxy_runtime::Call::AcurastProxy;

            let message_call = AcurastProxy(register {
                registration: JobRegistration {
                    script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
                    allowed_sources: None,
                    allow_only_verified_sources: false,
                    extra: (),
                },
            });
            let alice_origin = proxy_runtime::Origin::signed(ALICE);
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::JobRegistrationStored;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Event, Runtime, System};

            let events = System::events();
            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let _p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
            assert!(events
                .iter()
                .any(|event| matches!(event.event, Event::Acurast(JobRegistrationStored { .. }))));
        });
    }

    #[test]
    fn deregister() {
        Network::reset();
        register();

        // check that job is stored in the context of this test
        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::Runtime;

            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
            assert!(p_store.is_some());
        });

        use frame_support::dispatch::Dispatchable;
        use hex_literal::hex;
        use pallet_acurast::{JobRegistration, Script};

        CumulusParachain::execute_with(|| {
            use crate::pallet::Call::deregister;
            use proxy_runtime::Call::AcurastProxy;

            let message_call = AcurastProxy(deregister {
                script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
            });

            let alice_origin = proxy_runtime::Origin::signed(ALICE);
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::JobRegistrationRemoved;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Event, Runtime, System};

            let events = System::events();
            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let _p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
            assert!(events
                .iter()
                .any(|event| matches!(event.event, Event::Acurast(JobRegistrationRemoved { .. }))));
        });
    }

    #[test]
    fn update_allowed_sources() {
        Network::reset();

        register();

        // check that job is stored in the context of this test
        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::Runtime;

            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
            assert!(p_store.is_some());
        });

        use frame_support::dispatch::Dispatchable;
        use hex_literal::hex;
        use pallet_acurast::{AllowedSourcesUpdate, Script};

        let rand_array: [u8; 32] = rand::random();
        let source = sp_runtime::AccountId32::new(rand_array);

        CumulusParachain::execute_with(|| {
            use crate::pallet::Call::update_allowed_sources;
            use proxy_runtime::Call::AcurastProxy;

            let update = AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: source.clone(),
            };

            let message_call = AcurastProxy(update_allowed_sources {
                script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
                updates: vec![update],
            });

            let alice_origin = proxy_runtime::Origin::signed(ALICE);
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::AllowedSourcesUpdated;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Event, Runtime, System};

            let events = System::events();
            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);

            // source in storage same as one submitted to proxy
            let found_source: &sp_runtime::AccountId32 =
                &p_store.unwrap().allowed_sources.unwrap()[0];
            assert_eq!(*found_source, source);

            // event emitted
            assert!(events
                .iter()
                .any(|event| matches!(event.event, Event::Acurast(AllowedSourcesUpdated { .. }))));
        });
    }

    #[test]
    fn fulfill() {
        Network::reset();

        register();

        // check that job is stored in the context of this test
        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::Runtime;

            let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
            let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
            assert!(p_store.is_some());
        });

        use frame_support::dispatch::Dispatchable;
        use hex_literal::hex;
        use pallet_acurast::{AllowedSourcesUpdate, Script};

        let rand_array: [u8; 32] = rand::random();
        let BOB = sp_runtime::AccountId32::new(rand_array);

        CumulusParachain::execute_with(|| {
            use crate::pallet::Call::fulfill;
            use proxy_runtime::Call::AcurastProxy;

            let payload: [u8; 32] = rand::random();

            let fulfillment = Fulfillment {
                script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
                payload: payload.to_vec(),
            };

            let message_call = AcurastProxy(fulfill {
                fulfillment,
                requester: sp_runtime::MultiAddress::Id(ALICE),
            });

            let bob_origin = proxy_runtime::Origin::signed(BOB);
            let dispatch_status = message_call.dispatch(bob_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::ReceivedFulfillment;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Event, Runtime, System};

            let events = System::events();

            //event emitted
            assert!(events
                .iter()
                .any(|event| matches!(event.event, Event::Acurast(ReceivedFulfillment { .. }))));
        });
    }

    #[test]
    fn submit_attestation() {
        Network::reset();

        const ROOT_CERT: [u8; 1380] = hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4");
        const INT_CERT_1: [u8; 987] = hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42");
        const INT_CERT_2: [u8; 564] = hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb");
        const LEAF_CERT: [u8; 672] = hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9");

        let attestation_chain = AttestationChain {
            certificate_chain: vec![
                ROOT_CERT.to_vec().try_into().unwrap(),
                INT_CERT_1.to_vec().try_into().unwrap(),
                INT_CERT_2.to_vec().try_into().unwrap(),
                LEAF_CERT.to_vec().try_into().unwrap(),
            ]
            .try_into()
            .unwrap(),
        };

        AcurastParachain::execute_with(|| {
            use acurast_runtime::{Origin, Timestamp};
            _ = Timestamp::set(Origin::none(), 1657363915001).expect("Couldn't set timestamp");
        });

        CumulusParachain::execute_with(|| {
            use crate::pallet::Call::submit_attestation;
            use proxy_runtime::Call::AcurastProxy;
            use proxy_runtime::{Origin, Runtime};

            let message_call = AcurastProxy(submit_attestation { attestation_chain });

            let x_origin = Origin::signed(
                hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into(),
            )
            .into();
            let dispatch_status = message_call.dispatch(x_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::AttestationStored;
            use acurast_runtime::{Acurast, Event, Runtime, System};

            let events = System::events();

            // attestation stored
            let _p_store = Acurast::stored_attestation(ALICE);

            // event emitted
            assert!(events
                .iter()
                .any(|event| matches!(event.event, Event::Acurast(AttestationStored { .. }))));
        });
    }
}
