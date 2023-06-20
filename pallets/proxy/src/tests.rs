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

use frame_support::sp_runtime::traits::AccountIdConversion;
use hex_literal::hex;
use polkadot_parachain::primitives::Id as ParaId;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

use acurast_runtime::AccountId as AcurastAccountId;
use acurast_runtime::Runtime as AcurastRuntime;
use pallet_acurast::MultiOrigin;
use pallet_acurast_marketplace::FeeManager;

use crate::mock::acurast_runtime::FeeManagerImpl;
use crate::mock::*;

pub type RelayChainPalletXcm = pallet_xcm::Pallet<relay_chain::Runtime>;
pub type AcurastPalletXcm = pallet_xcm::Pallet<acurast_runtime::Runtime>;

pub const ALICE: frame_support::sp_runtime::AccountId32 =
    frame_support::sp_runtime::AccountId32::new([0u8; 32]);
pub const BOB: frame_support::sp_runtime::AccountId32 =
    frame_support::sp_runtime::AccountId32::new([1u8; 32]);
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
    pub struct ProxyParachain {
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
            (2001, ProxyParachain),
        ],
    }
}

pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
    use acurast_runtime::{MsgQueue, Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (alice_account_id(), INITIAL_BALANCE),
            (pallet_fees_account(), INITIAL_BALANCE),
            (bob_account_id(), INITIAL_BALANCE),
            (processor_account_id(), INITIAL_BALANCE),
        ],
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

pub fn para_account_id(id: u32) -> relay_chain::AccountId {
    ParaId::from(id).into_account_truncating()
}
pub fn processor_account_id() -> AcurastAccountId {
    hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
}
pub fn pallet_fees_account() -> <AcurastRuntime as frame_system::Config>::AccountId {
    FeeManagerImpl::pallet_id().into_account_truncating()
}

#[cfg(test)]
mod network_tests {
    use codec::Encode;
    use frame_support::assert_ok;
    use xcm::latest::prelude::*;
    use xcm_simulator::{TestExt, Weight};

    use super::*;

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

        let remark = acurast_runtime::RuntimeCall::System(frame_system::Call::<
            acurast_runtime::Runtime,
        >::remark_with_event {
            remark: vec![1, 2, 3],
        });
        Relay::execute_with(|| {
            assert_ok!(RelayChainPalletXcm::send_xcm(
                Here,
                Parachain(2000),
                Xcm(vec![Transact {
                    origin_kind: OriginKind::SovereignAccount,
                    require_weight_at_most: Weight::from_parts(1_000_000_000, 0),
                    call: remark.encode().into(),
                }]),
            ));
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::{RuntimeEvent, System};
            assert!(System::events().iter().any(|r| matches!(
                r.event,
                RuntimeEvent::System(frame_system::Event::Remarked { .. })
            )));
        });
    }

    #[test]
    fn ump() {
        Network::reset();

        let remark = relay_chain::RuntimeCall::System(
            frame_system::Call::<relay_chain::Runtime>::remark_with_event {
                remark: vec![1, 2, 3],
            },
        );
        AcurastParachain::execute_with(|| {
            assert_ok!(AcurastPalletXcm::send_xcm(
                Here,
                Parent,
                Xcm(vec![Transact {
                    origin_kind: OriginKind::SovereignAccount,
                    require_weight_at_most: Weight::from_parts(1_000_000_000, 0),
                    call: remark.encode().into(),
                }]),
            ));
        });

        Relay::execute_with(|| {
            use relay_chain::{RuntimeEvent, System};
            assert!(System::events().iter().any(|r| matches!(
                r.event,
                RuntimeEvent::System(frame_system::Event::Remarked { .. })
            )));
        });
    }

    #[test]
    fn xcmp() {
        Network::reset();

        let remark = proxy_runtime::RuntimeCall::System(frame_system::Call::<
            proxy_runtime::Runtime,
        >::remark_with_event {
            remark: vec![1, 2, 3],
        });

        AcurastParachain::execute_with(|| {
            assert_ok!(AcurastPalletXcm::send_xcm(
                Here,
                (Parent, Parachain(2001)),
                Xcm(vec![Transact {
                    origin_kind: OriginKind::SovereignAccount,
                    require_weight_at_most: Weight::from_parts(1_000_000_000, 0),
                    call: remark.encode().into(),
                }]),
            ));
        });

        ProxyParachain::execute_with(|| {
            use proxy_runtime::{RuntimeEvent, System};
            assert!(System::events().iter().any(|r| matches!(
                r.event,
                RuntimeEvent::System(frame_system::Event::Remarked { .. })
            )));
        });
    }

    #[test]
    fn reserve_transfer() {
        Network::reset();

        let withdraw_amount = 123;

        Relay::execute_with(|| {
            assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
                relay_chain::RuntimeOrigin::signed(ALICE),
                Box::new(X1(Parachain(2000)).into()),
                Box::new(
                    X1(AccountId32 {
                        network: None,
                        id: ALICE.into()
                    })
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
}

#[cfg(test)]
mod proxy_calls {
    use acurast_common::JobIdSequence;
    use frame_support::assert_ok;
    use frame_support::dispatch::Dispatchable;
    use frame_support::pallet_prelude::Hooks;
    use pallet_acurast::LocalJobIdSequence;
    use xcm_simulator::TestExt;

    use super::*;

    #[test]
    fn register() {
        Network::reset();
        register_job_alice();
    }

    fn register_job_alice() -> JobIdSequence {
        ProxyParachain::execute_with(|| {
            use crate::pallet::Call::register;
            use proxy_runtime::RuntimeCall::AcurastProxy;

            let message_call = AcurastProxy(register {
                registration: registration(),
            });
            let alice_origin = proxy_runtime::RuntimeOrigin::signed(alice_account_id());
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::JobRegistrationStored;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Runtime, RuntimeEvent, System};

            let events = System::events();
            let multi_origin = MultiOrigin::Acurast(ALICE);
            let chain_job_id = LocalJobIdSequence::<Runtime>::get();
            let p_store = StoredJobRegistration::<Runtime>::get(multi_origin, chain_job_id);
            assert!(p_store.is_some());
            assert!(events.iter().any(|event| matches!(
                event.event,
                RuntimeEvent::Acurast(JobRegistrationStored { .. })
            )));
            chain_job_id
        })
    }

    #[test]
    fn deregister() {
        use frame_support::dispatch::Dispatchable;

        Network::reset();
        let chain_job_id = register_job_alice();

        // check that job is stored in the context of this test
        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::Runtime;

            let multi_origin = MultiOrigin::Acurast(ALICE);
            let p_store = StoredJobRegistration::<Runtime>::get(multi_origin, chain_job_id.clone());
            assert!(p_store.is_some());

            later(registration().schedule.start_time + 3000); // pretend actual execution until report call took 3 seconds
        });

        ProxyParachain::execute_with(|| {
            use crate::pallet::Call::deregister;
            use proxy_runtime::RuntimeCall::AcurastProxy;

            let job_id = 1;
            let message_call = AcurastProxy(deregister { job_id });

            let alice_origin = proxy_runtime::RuntimeOrigin::signed(ALICE);
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::JobRegistrationRemoved;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Runtime, RuntimeEvent, System};

            let multi_origin = MultiOrigin::Acurast(ALICE);
            let p_store = StoredJobRegistration::<Runtime>::get(multi_origin, chain_job_id);
            assert!(p_store.is_none());

            let events = System::events();
            assert!(events.iter().any(|event| matches!(
                event.event,
                RuntimeEvent::Acurast(JobRegistrationRemoved { .. })
            )));
        });
    }

    #[test]
    fn update_allowed_sources() {
        Network::reset();

        let chain_job_id = register_job_alice();

        // check that job is stored in the context of this test
        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::Runtime;

            let multi_origin = MultiOrigin::Acurast(ALICE);
            let p_store = StoredJobRegistration::<Runtime>::get(multi_origin, chain_job_id);
            assert!(p_store.is_some());
        });

        let rand_array: [u8; 32] = rand::random();
        let source = frame_support::sp_runtime::AccountId32::new(rand_array);

        ProxyParachain::execute_with(|| {
            use crate::pallet::Call::update_allowed_sources;
            use pallet_acurast::{AllowedSourcesUpdate, ListUpdateOperation};
            use proxy_runtime::RuntimeCall::AcurastProxy;

            let update = AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                item: source.clone(),
            };

            let message_call = AcurastProxy(update_allowed_sources {
                job_id: 1,
                updates: vec![update].try_into().unwrap(),
            });

            let alice_origin = proxy_runtime::RuntimeOrigin::signed(ALICE);
            let dispatch_status = message_call.dispatch(alice_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast::Event::AllowedSourcesUpdated;
            use acurast_runtime::pallet_acurast::StoredJobRegistration;
            use acurast_runtime::{Runtime, RuntimeEvent, System};

            let events = System::events();
            let multi_origin = MultiOrigin::Acurast(ALICE);
            let p_store = StoredJobRegistration::<Runtime>::get(multi_origin, chain_job_id);

            // source in storage same as one submitted to proxy
            let found_source: &frame_support::sp_runtime::AccountId32 =
                &p_store.unwrap().allowed_sources.unwrap()[0];
            assert_eq!(*found_source, source);

            // event emitted
            assert!(events.iter().any(|event| matches!(
                event.event,
                RuntimeEvent::Acurast(AllowedSourcesUpdated { .. })
            )));
        });
    }

    #[test]
    fn advertise() {
        advertise_bob();
    }

    fn advertise_bob() {
        Network::reset();

        ProxyParachain::execute_with(|| {
            use crate::pallet::Call::advertise;
            use proxy_runtime::RuntimeCall::AcurastProxy;

            let message_call = AcurastProxy(advertise {
                advertisement: advertisement(10000u128),
            });
            let bob_origin = proxy_runtime::RuntimeOrigin::signed(bob_account_id());
            let dispatch_status = message_call.dispatch(bob_origin);
            assert_ok!(dispatch_status);
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::pallet_acurast_marketplace::Event::AdvertisementStored;
            use acurast_runtime::pallet_acurast_marketplace::StoredAdvertisementRestriction;
            use acurast_runtime::{Runtime, RuntimeEvent, System};

            let events = System::events();
            let p_store = StoredAdvertisementRestriction::<Runtime>::get(BOB);
            assert!(p_store.is_some());
            assert!(events.iter().any(|event| matches!(
                event.event,
                RuntimeEvent::AcurastMarketplace(AdvertisementStored { .. })
            )));
        });
    }

    fn next_block() {
        if acurast_runtime::System::block_number() >= 1 {
            // pallet_acurast_marketplace::on_finalize(System::block_number());
            acurast_runtime::Timestamp::on_finalize(acurast_runtime::System::block_number());
        }
        acurast_runtime::System::set_block_number(acurast_runtime::System::block_number() + 1);
        acurast_runtime::Timestamp::on_initialize(acurast_runtime::System::block_number());
    }

    /// A helper function to move time on in tests. It ensures `acurast_runtime::Timestamp::set` is only called once per block by advancing the block otherwise.
    fn later(now: u64) {
        // If this is not the very first timestamp ever set, we always advance the block before setting new time
        // this is because setting it twice in a block is not legal
        if acurast_runtime::Timestamp::get() > 0 {
            // pretend block was finalized
            let b = acurast_runtime::System::block_number();
            next_block(); // we cannot set time twice in same block
            assert_eq!(b + 1, acurast_runtime::System::block_number());
        }
        // pretend time moved on
        assert_ok!(acurast_runtime::Timestamp::set(
            acurast_runtime::RuntimeOrigin::none(),
            now
        ));
    }
}
