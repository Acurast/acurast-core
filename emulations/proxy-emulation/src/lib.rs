// parent re-exports
use emulations::runtimes::{proxy_runtime, acurast_runtime, polkadot_runtime};
use emulations::emulators::{xcm_emulator};

// needed libs
use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};
use frame_support::traits::GenesisBuild;
use sp_runtime::AccountId32;

// decl_test_relay_chain! {
// 	pub struct PolkadotRelay {
// 		Runtime = polkadot_runtime::Runtime,
// 		XcmConfig = polkadot_runtime::xcm_config::XcmConfig,
// 		new_ext = polkadot_ext(),
// 	}
// }
//
// decl_test_parachain! {
// 	pub struct AcurastParachain {
// 		Runtime = acurast_runtime::Runtime,
// 		Origin = acurast_runtime::Origin,
// 		XcmpMessageHandler = acurast_runtime::XcmpQueue,
// 		DmpMessageHandler = acurast_runtime::DmpQueue,
// 		new_ext = acurast_ext(2000),
// 	}
// }
//
// decl_test_parachain! {
// 	pub struct ProxyParachain {
// 		Runtime = proxy_runtime::Runtime,
// 		Origin = proxy_runtime::Origin,
// 		XcmpMessageHandler = proxy_runtime::XcmpQueue,
// 		DmpMessageHandler = proxy_runtime::DmpQueue,
// 		new_ext = cumulus_ext(2001),
// 	}
// }

// decl_test_parachain! {
// 	pub struct StatemintParachain {
// 		Runtime = statemint_runtime::Runtime,
// 		Origin = statemint_runtime::Origin,
// 		XcmpMessageHandler = statemint_runtime::XcmpQueue,
// 		DmpMessageHandler = statemint_runtime::DmpQueue,
// 		new_ext = cumulus_ext(1000),
// 	}
// }

// decl_test_network! {
// 	pub struct Network {
// 		relay_chain = PolkadotRelay,
// 		parachains = vec![
// 			(2000, AcurastParachain),
// 			(2001, ProxyParachain),
// 			(1000, StatemintParachain),
// 		],
// 	}
// }
//
// pub const ALICE: AccountId32 = AccountId32::new([0u8; 32]);
// pub const INITIAL_BALANCE: u128 = 1_000_000_000_000;
//
// pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
// 	use acurast_runtime::{Runtime, System};
//
// 	let mut t = frame_system::GenesisConfig::default()
// 		.build_storage::<Runtime>()
// 		.unwrap();
//
// 	let parachain_info_config = parachain_info::GenesisConfig {
// 		parachain_id: para_id.into(),
// 	};
//
// 	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
// 		.unwrap();
//
// 	pallet_balances::GenesisConfig::<Runtime> {
// 		balances: vec![(ALICE, INITIAL_BALANCE)],
// 	}
// 	.assimilate_storage(&mut t)
// 	.unwrap();
//
// 	let mut ext = sp_io::TestExternalities::new(t);
// 	ext.execute_with(|| System::set_block_number(1));
// 	ext
// }
//
// pub fn cumulus_ext(para_id: u32) -> sp_io::TestExternalities {
// 	use parachain_template_runtime::{Runtime, System};
//
// 	let mut t = frame_system::GenesisConfig::default()
// 		.build_storage::<Runtime>()
// 		.unwrap();
//
// 	let parachain_info_config = parachain_info::GenesisConfig {
// 		parachain_id: para_id.into(),
// 	};
//
// 	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
// 		.unwrap();
//
// 	pallet_balances::GenesisConfig::<Runtime> {
// 		balances: vec![(ALICE, INITIAL_BALANCE)],
// 	}
// 		.assimilate_storage(&mut t)
// 		.unwrap();
//
// 	let mut ext = sp_io::TestExternalities::new(t);
// 	ext.execute_with(|| System::set_block_number(1));
// 	ext
// }
//
// pub fn statemint_ext(para_id: u32) -> sp_io::TestExternalities {
// 	use statemint_runtime::{Runtime, System};
//
// 	let mut t = frame_system::GenesisConfig::default()
// 		.build_storage::<Runtime>()
// 		.unwrap();
//
// 	let parachain_info_config = parachain_info::GenesisConfig {
// 		parachain_id: para_id.into(),
// 	};
//
// 	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
// 		.unwrap();
//
// 	pallet_balances::GenesisConfig::<Runtime> {
// 		balances: vec![(ALICE, INITIAL_BALANCE)],
// 	}
// 		.assimilate_storage(&mut t)
// 		.unwrap();
//
// 	let pallet_xcm_config = pallet_xcm::GenesisConfig::default();
// 		<pallet_xcm::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&pallet_xcm_config, &mut t)
// 		.unwrap();
//
// 	let mut ext = sp_io::TestExternalities::new(t);
// 	ext.execute_with(|| System::set_block_number(1));
// 	ext
// }
//
// fn default_parachains_host_configuration(
// ) -> polkadot_runtime_parachains::configuration::HostConfiguration<polkadot_primitives::v2::BlockNumber> {
// 	use polkadot_primitives::v2::{MAX_CODE_SIZE, MAX_POV_SIZE};
//
// 	polkadot_runtime_parachains::configuration::HostConfiguration {
// 		minimum_validation_upgrade_delay: 5,
// 		validation_upgrade_cooldown: 10u32,
// 		validation_upgrade_delay: 10,
// 		code_retention_period: 1200,
// 		max_code_size: MAX_CODE_SIZE,
// 		max_pov_size: MAX_POV_SIZE,
// 		max_head_data_size: 32 * 1024,
// 		group_rotation_frequency: 20,
// 		chain_availability_period: 4,
// 		thread_availability_period: 4,
// 		max_upward_queue_count: 8,
// 		max_upward_queue_size: 1024 * 1024,
// 		max_downward_message_size: 1024,
// 		ump_service_total_weight: 4 * 1_000_000_000,
// 		max_upward_message_size: 50 * 1024,
// 		max_upward_message_num_per_candidate: 5,
// 		hrmp_sender_deposit: 0,
// 		hrmp_recipient_deposit: 0,
// 		hrmp_channel_max_capacity: 8,
// 		hrmp_channel_max_total_size: 8 * 1024,
// 		hrmp_max_parachain_inbound_channels: 4,
// 		hrmp_max_parathread_inbound_channels: 4,
// 		hrmp_channel_max_message_size: 1024 * 1024,
// 		hrmp_max_parachain_outbound_channels: 4,
// 		hrmp_max_parathread_outbound_channels: 4,
// 		hrmp_max_message_num_per_candidate: 5,
// 		dispute_period: 6,
// 		no_show_slots: 2,
// 		n_delay_tranches: 25,
// 		needed_approvals: 2,
// 		relay_vrf_modulo_samples: 2,
// 		zeroth_delay_tranche_width: 0,
// 		..Default::default()
// 	}
// }
//
// pub fn polkadot_ext() -> sp_io::TestExternalities {
// 	use polkadot_runtime::{Runtime, System};
//
// 	let mut t = frame_system::GenesisConfig::default()
// 		.build_storage::<Runtime>()
// 		.unwrap();
//
// 	pallet_balances::GenesisConfig::<Runtime> {
// 		balances: vec![(ALICE, INITIAL_BALANCE)],
// 	}
// 	.assimilate_storage(&mut t)
// 	.unwrap();
//
// 	polkadot_runtime_parachains::configuration::GenesisConfig::<Runtime> {
// 		config: default_parachains_host_configuration(),
// 	}
// 	.assimilate_storage(&mut t)
// 	.unwrap();
//
// 	let mut ext = sp_io::TestExternalities::new(t);
// 	ext.execute_with(|| System::set_block_number(1));
// 	ext
// }
//
// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use codec::{Decode, Encode};
//
// 	use cumulus_primitives_core::ParaId;
// 	use frame_support::{assert_ok, RuntimeDebug, traits::Currency};
// 	use frame_support::dispatch::TypeInfo;
// 	use sp_runtime::traits::{AccountIdConversion};
// 	use xcm::{latest::prelude::*};
// 	// use xcm::v2::{MultiLocation};
// 	use frame_support::traits::PalletInfoAccess;
// 	use polkadot_parachain::primitives::Sibling;
// 	use xcm_emulator::TestExt;
// 	type CumulusXcmPallet = pallet_xcm::Pallet<parachain_template_runtime::Runtime>;
// 	type AcurastXcmPallet = pallet_xcm::Pallet<acurast_runtime::Runtime>;
// 	type PolkadotXcmPallet = pallet_xcm::Pallet<polkadot_runtime::Runtime>;
// 	type StatemintXcmPallet = pallet_xcm::Pallet<statemint_runtime::Runtime>;
//
// 	type StatemintMinter = pallet_assets::Pallet<statemint_runtime::Runtime>;
// 	type AcurastMinter = pallet_assets::Pallet<statemint_runtime::Runtime>;
//
// 	#[test]
// 	fn dmp() {
// 		Network::reset();
//
// 		let remark = acurast_runtime::Call::System(frame_system::Call::<acurast_runtime::Runtime>::remark_with_event {
// 			remark: "Hello from Atera".as_bytes().to_vec(),
// 		});
// 		PolkadotRelay::execute_with(|| {
// 			assert_ok!(polkadot_runtime::XcmPallet::force_default_xcm_version(
// 				polkadot_runtime::Origin::root(),
// 				Some(0)
// 			));
// 			assert_ok!(polkadot_runtime::XcmPallet::send_xcm(
// 				Here,
// 				Parachain(2000),
// 				Xcm(vec![Transact {
// 					origin_type: OriginKind::SovereignAccount,
// 					require_weight_at_most: INITIAL_BALANCE as u64,
// 					call: remark.encode().into(),
// 				}]),
// 			));
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System};
// 			System::events().iter().for_each(|r| println!(">>> {:?}", r.event));
//
// 			assert!(System::events().iter().any(|r| matches!(
// 				r.event,
// 				Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
// 			)));
// 		});
// 	}
//
// 	#[test]
// 	fn ump() {
// 		Network::reset();
//
// 		PolkadotRelay::execute_with(|| {
// 			let _ = polkadot_runtime::Balances::deposit_creating(
// 				&ParaId::from(2000).into_account_truncating(),
// 				1_000_000_000_000,
// 			);
// 		});
//
// 		let remark = polkadot_runtime::Call::System(frame_system::Call::<polkadot_runtime::Runtime>::remark_with_event {
// 			remark: "Hello from Acurast!".as_bytes().to_vec(),
// 		});
//
// 		let send_amount = 1_000_000_000_000;
// 		AcurastParachain::execute_with(|| {
// 			assert_ok!(acurast_runtime::PolkadotXcm::send_xcm(
// 				Here,
// 				Parent,
// 				Xcm(vec![
// 						WithdrawAsset((Here, send_amount).into()),
// 						buy_execution((Here, send_amount)),
// 						Transact {
// 							origin_type: OriginKind::SovereignAccount,
// 							require_weight_at_most: INITIAL_BALANCE as u64,
// 							call: remark.encode().into(),
// 						}
// 				]),
// 			));
// 		});
//
// 		PolkadotRelay::execute_with(|| {
// 			use polkadot_runtime::{Event, System};
// 			let _event_list = System::events();
// 			assert!(System::events().iter().any(|r| matches!(
// 				r.event,
// 				Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
// 			)));
// 		});
// 	}
//
// 	#[test]
// 	fn xcmp() {
// 		Network::reset();
//
// 		let remark = parachain_template_runtime::Call::System(frame_system::Call::<parachain_template_runtime::Runtime>::remark_with_event {
// 			remark: "Hello from acurast!".as_bytes().to_vec(),
// 		});
// 		AcurastParachain::execute_with(|| {
// 			assert_ok!(acurast_runtime::PolkadotXcm::send_xcm(
// 				Here,
// 				MultiLocation::new(1, X1(Parachain(2001))),
// 				Xcm(vec![Transact {
// 					origin_type: OriginKind::SovereignAccount,
// 					require_weight_at_most: 10_000_000,
// 					call: remark.encode().into(),
// 				}]),
// 			));
// 		});
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::{Event, System};
// 			System::events().iter().for_each(|r| println!(">>> {:?}", r.event));
//
// 			assert!(System::events().iter().any(|r| matches!(
// 				r.event,
// 				Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
// 			)));
// 		});
// 	}
// 		// individual holding assets in their statemint chain account, sends a reserve transfer of DOT
// 	// to an individual on the acurast chain
// 	#[test]
// 	fn dot_reserve_transfer() {
// 		Network::reset();
// 		StatemintParachain::execute_with(|| {
// 			let _alice_balance = pallet_balances::Pallet::<statemint_runtime::Runtime>::free_balance(&ALICE);
// 			let xcm = StatemintXcmPallet::reserve_transfer_assets(
// 				statemint_runtime::Origin::signed(ALICE),
// 				Box::new(MultiLocation{parents: 1, interior: X1(Parachain(2000))}.into()),
// 				Box::new(X1(Junction::AccountId32 {
// 					network: NetworkId::Any,
// 					id: ALICE.into()
// 				}).into().into()),
// 				Box::new(vec![
// 					MultiAsset {
// 						id: Concrete(Parent.into()),
// 						fun: Fungible(INITIAL_BALANCE/2)
// 					},
// 				].into()),
// 				0,
// 			);
// 			assert_ok!(xcm);
// 		});
//
// 		StatemintParachain::execute_with(|| {
// 			let _events = statemint_runtime::System::events();
// 			println!("events!")
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			let _events = acurast_runtime::System::events();
// 			println!("events!")
// 		});
//
//
// 		AcurastParachain::execute_with(|| {
// 			let alice_balance = pallet_balances::Pallet::<acurast_runtime::Runtime>::free_balance(&ALICE);
// 			assert!(alice_balance < INITIAL_BALANCE + INITIAL_BALANCE/2 && alice_balance > INITIAL_BALANCE)
// 		})
// 	}
//
// 	#[test]
// 	fn mint_new_fungible() {
// 		Network::reset();
// 		StatemintParachain::execute_with(|| {
// 			let result = StatemintMinter::create(
// 				statemint_runtime::Origin::signed(ALICE),
// 				1,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				10
// 			);
// 			assert_ok!(result);
//
// 			let result = StatemintMinter::mint(
// 				statemint_runtime::Origin::signed(ALICE),
// 				1,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				1500
// 			);
// 			assert_ok!(result);
//
// 			let alice_balance = StatemintMinter::balance(
// 				1,
// 				&ALICE,
// 			);
//
// 			assert_eq!(alice_balance, 1500);
// 		})
// 	}
//
// 	#[test]
// 	fn new_fungible_reserve_transfer() {
// 		Network::reset();
//
// 		let acurast_sovereign: sp_runtime::AccountId32 = Sibling::from(2000).into_account_truncating();
// 		// create and mint to alice new fungible with id 1
// 		StatemintParachain::execute_with(|| {
// 			let _alice_balance = pallet_balances::Pallet::<statemint_runtime::Runtime>::free_balance(&ALICE);
// 			// create token 1
// 			let result = StatemintMinter::create(
// 				statemint_runtime::Origin::signed(ALICE),
// 				1,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				10
// 			);
// 			assert_ok!(result);
//
// 			// mint 1500 to alice
// 			let result = StatemintMinter::mint(
// 				statemint_runtime::Origin::signed(ALICE),
// 				1,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				1500
// 			);
// 			assert_ok!(result);
// 			let alice_balance = StatemintMinter::balance(
// 				1,
// 				&ALICE,
// 			);
// 			assert_eq!(alice_balance, 1500);
//
// 			// acurast sovereign account needs a minimum balance of DOT to be a valid account.
// 			let deposit_result = statemint_runtime::Balances::deposit_creating(
// 				&acurast_sovereign,
// 				1_000_000_000_000,
// 			);
// 			let acurast_balance = statemint_runtime::Balances::total_balance(&acurast_sovereign);
// 			assert_eq!(acurast_balance, 1_000_000_000_000);
//
// 			// mint 1500 of token 1 to acurast sovereign acc
// 			let result = StatemintMinter::mint(
// 				statemint_runtime::Origin::signed(ALICE),
// 				1,
// 				sp_runtime::MultiAddress::Id(acurast_sovereign.clone()),
// 				1500
// 			);
// 			assert_ok!(result);
// 			let acurast_token_balance = StatemintMinter::balance(
// 				1,
// 				&acurast_sovereign,
// 			);
// 			assert_eq!(acurast_token_balance, 1500);
// 		});
//
// 		// create same asset2 in acurast
// 		AcurastParachain::execute_with(|| {
// 			let result = AcurastMinter::create(
// 				statemint_runtime::Origin::signed(ALICE),
// 				2,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				10
// 			);
// 			assert_ok!(result);
// 			// mint 1500 to alice
// 			let result = StatemintMinter::mint(
// 				statemint_runtime::Origin::signed(ALICE),
// 				2,
// 				sp_runtime::MultiAddress::Id(ALICE),
// 				1500
// 			);
// 		});
//
//
// 		// reserve backed transfer of token 1 from statemint to acurast
// 		StatemintParachain::execute_with(|| {
// 			let xcm = StatemintXcmPallet::reserve_transfer_assets(
// 				statemint_runtime::Origin::signed(ALICE),
// 				Box::new(MultiLocation{parents: 1, interior: X1(Parachain(2000))}.into()),
// 				Box::new(X1(Junction::AccountId32 {
// 					network: NetworkId::Any,
// 					id: ALICE.into()
// 				}).into().into()),
// 				Box::new(vec![
//
// 					MultiAsset {
// 						id: Concrete(X2(PalletInstance(50), GeneralIndex(1)).into()),
// 						fun: Fungible(500)
// 					},
// 					MultiAsset {
// 						id: Concrete(Parent.into()),
// 						fun: Fungible(INITIAL_BALANCE/10)
// 					},
// 					// MultiAsset {
// 					// 	id: Concrete(X2(PalletInstance(50), GeneralIndex(2)).into()),
// 					// 	fun: Fungible(500)
// 					// },
//
// 				].into()),
// 				1,
// 			);
// 			assert_ok!(xcm);
// 		});
//
// 		StatemintParachain::execute_with(|| {
// 			let _events = statemint_runtime::System::events();
// 			println!("stop");
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			let _events = acurast_runtime::System::events();
// 			let alice_balance = AcurastMinter::balance(1, &ALICE);
// 			assert_eq!(alice_balance, 500);
// 		})
//
// 	}
// 	// Helper function for forming buy execution message
// 	fn buy_execution<C>(fees: impl Into<MultiAsset>) -> Instruction<C> {
// 		BuyExecution { fees: fees.into(), weight_limit: Unlimited }
// 	}
// }
//
// #[cfg(test)]
// mod proxy_calls {
// 	use super::*;
// 	use frame_support::{assert_ok};
// 	use pallet_acurast::{AllowedSourcesUpdate, ListUpdateOperation, Fulfillment, AttestationChain};
// 	use xcm_emulator::TestExt;
// 	use hex_literal::hex;
// 	use frame_support::dispatch::Dispatchable;
//
// 	const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
//
// 	#[test]
// 	fn register() {
// 		Network::reset();
// 		use pallet_acurast::{JobRegistration, Script};
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::Call::AcurastProxy;
// 			use acurast_proxy::pallet::Call::register;
//
// 			let message_call =  AcurastProxy( register {
// 				registration: JobRegistration {
// 						script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
// 						allowed_sources: None,
// 						allow_only_verified_sources: false,
// 						extra: ()
// 					}
// 			} );
// 			let alice_origin = parachain_template_runtime::Origin::signed(ALICE);
// 			let dispatch_status = message_call.dispatch(alice_origin);
// 			assert_ok!(dispatch_status);
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System, Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
// 			use acurast_runtime::pallet_acurast::Event::JobRegistrationStored;
//
// 			let events = System::events();
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let _p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
// 			assert!(
// 				events.iter().any(|event|
// 					matches!(event.event, Event::Acurast(JobRegistrationStored{ .. }))
// 				)
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn deregister() {
// 		Network::reset();
// 		register();
//
// 		// check that job is stored in the context of this test
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
//
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
// 			assert!(p_store.is_some());
// 		});
//
// 		use frame_support::dispatch::Dispatchable;
// 		use hex_literal::hex;
// 		use pallet_acurast::{JobRegistration, Script};
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::Call::AcurastProxy;
// 			use acurast_proxy::pallet::Call::deregister;
//
// 			let message_call =  AcurastProxy( deregister {
// 				script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
// 			});
//
// 			let alice_origin = parachain_template_runtime::Origin::signed(ALICE);
// 			let dispatch_status = message_call.dispatch(alice_origin);
// 			assert_ok!(dispatch_status);
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System, Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
// 			use acurast_runtime::pallet_acurast::Event::JobRegistrationRemoved;
//
// 			let events = System::events();
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let _p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
// 			assert!(
// 				events.iter().any(|event|
// 					matches!(event.event, Event::Acurast(JobRegistrationRemoved{ .. }))
// 				)
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn update_allowed_sources() {
// 		Network::reset();
//
// 		register();
//
// 		// check that job is stored in the context of this test
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
//
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
// 			assert!(p_store.is_some());
// 		});
//
// 		use frame_support::dispatch::Dispatchable;
// 		use hex_literal::hex;
// 		use pallet_acurast::{Script, AllowedSourcesUpdate};
//
//
// 		let rand_array: [u8; 32] = rand::random();
// 		let source = sp_runtime::AccountId32::new(rand_array);
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::Call::AcurastProxy;
// 			use acurast_proxy::pallet::Call::update_allowed_sources;
//
//
// 			let update = AllowedSourcesUpdate { operation: ListUpdateOperation::Add, account_id: source.clone() };
//
// 			let message_call =  AcurastProxy( update_allowed_sources {
// 				script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
// 				updates: vec![update]
// 			});
//
// 			let alice_origin = parachain_template_runtime::Origin::signed(ALICE);
// 			let dispatch_status = message_call.dispatch(alice_origin);
// 			assert_ok!(dispatch_status);
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System, Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
// 			use acurast_runtime::pallet_acurast::Event::AllowedSourcesUpdated;
//
// 			let events = System::events();
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
//
// 			// source in storage same as one submitted to proxy
// 			let found_source: &sp_runtime::AccountId32 = &p_store.unwrap().allowed_sources.unwrap()[0];
// 			assert_eq!(*found_source, source);
//
// 			// event emitted
// 			assert!(
// 				events.iter().any(|event|
// 					matches!(event.event, Event::Acurast(AllowedSourcesUpdated{ .. }))
// 				)
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn fulfill() {
// 		Network::reset();
//
// 		register();
//
// 		// check that job is stored in the context of this test
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
//
// 			let script: Script = SCRIPT_BYTES.to_vec().try_into().unwrap();
// 			let p_store = StoredJobRegistration::<Runtime>::get(ALICE, script);
// 			assert!(p_store.is_some());
// 		});
//
// 		use frame_support::dispatch::Dispatchable;
// 		use hex_literal::hex;
// 		use pallet_acurast::{Script, AllowedSourcesUpdate};
//
// 		let rand_array: [u8; 32] = rand::random();
// 		let BOB = sp_runtime::AccountId32::new(rand_array);
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::Call::AcurastProxy;
// 			use acurast_proxy::pallet::Call::fulfill;
//
// 			let payload: [u8; 32] = rand::random();
//
// 			let fulfillment = Fulfillment{
// 				script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
// 				payload: payload.to_vec()
// 			};
//
// 			let message_call =  AcurastProxy( fulfill {
// 				fulfillment,
// 				requester: sp_runtime::MultiAddress::Id(ALICE)
// 			});
//
// 			let bob_origin = parachain_template_runtime::Origin::signed(BOB);
// 			let dispatch_status = message_call.dispatch(bob_origin);
// 			assert_ok!(dispatch_status);
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System, Runtime};
// 			use acurast_runtime::pallet_acurast::StoredJobRegistration;
// 			use acurast_runtime::pallet_acurast::Event::ReceivedFulfillment;
//
// 			let events = System::events();
//
// 			//event emitted
// 			assert!(
// 				events.iter().any(|event|
// 					matches!(event.event, Event::Acurast(ReceivedFulfillment{ .. }))
// 				)
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn submit_attestation(){
// 		Network::reset();
//
// 		const ROOT_CERT: [u8; 1380] = hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4");
// 		const INT_CERT_1: [u8; 987] = hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42");
// 		const INT_CERT_2: [u8; 564] = hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb");
// 		const LEAF_CERT: [u8; 672] = hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9");
//
// 		let attestation_chain = AttestationChain {
// 			certificate_chain: vec![
// 				ROOT_CERT.to_vec().try_into().unwrap(),
// 				INT_CERT_1.to_vec().try_into().unwrap(),
// 				INT_CERT_2.to_vec().try_into().unwrap(),
// 				LEAF_CERT.to_vec().try_into().unwrap(),
// 			]
// 				.try_into()
// 				.unwrap(),
// 		};
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Timestamp, Origin};
// 			_ = Timestamp::set(Origin::none(), 1657363915001).expect("timestamping failed");
// 		});
//
// 		CumulusParachain::execute_with(|| {
// 			use parachain_template_runtime::{Origin, Runtime};
// 			use parachain_template_runtime::Call::AcurastProxy;
// 			use acurast_proxy::pallet::Call::submit_attestation;
//
// 			let message_call = AcurastProxy(submit_attestation {
// 				attestation_chain
// 			});
//
// 			let x_origin = Origin::signed(ALICE).into();
// 			let dispatch_status = message_call.dispatch(x_origin);
// 			assert_ok!(dispatch_status);
// 		});
//
// 		AcurastParachain::execute_with(|| {
// 			use acurast_runtime::{Event, System, Runtime, Acurast};
// 			use acurast_runtime::pallet_acurast::StoredAttestation;
// 			use acurast_runtime::pallet_acurast::Event::AttestationStored;
//
// 			let events = System::events();
//
// 			// attestation stored
// 			let _p_store = Acurast::stored_attestation(ALICE);
//
// 			// event emitted
// 			assert!(
// 				events.iter().any(|event|
// 					matches!(event.event, Event::Acurast(AttestationStored{ .. }))
// 				)
// 			);
// 		});
//
// 	}
//
// }