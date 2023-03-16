//! Benchmarking setup for pallet-acurast-processor-manager

use crate::stub::{alice_account_id, generate_account};

use super::*;

use acurast_common::ListUpdateOperation;
use codec::Encode;
use frame_benchmarking::{benchmarks, whitelist_account};
use frame_support::{
    sp_runtime::{
        traits::{IdentifyAccount, StaticLookup, Verify},
        AccountId32, MultiSignature,
    },
    traits::{Get, IsType},
};
use frame_system::RawOrigin;
use sp_core::Pair;
use sp_std::prelude::*;

fn generate_pairing_update<T: Config<AccountId = AccountId32, Proof = MultiSignature>>(
    operation: ListUpdateOperation,
    caller: &T::AccountId,
) -> ProcessorPairingUpdateFor<T> {
    let (processor_pair, processor_account_id) = generate_account();
    let timestamp = 1657363915002u128;
    let message = [caller.encode(), timestamp.encode(), 1u128.encode()].concat();
    let signature: MultiSignature = processor_pair.sign(&message).into();
    ProcessorPairingUpdateFor::<T> {
        operation,
        item: ProcessorPairingFor::<T>::new_with_proof(processor_account_id, timestamp, signature),
    }
}

benchmarks! {
    where_clause { where
        T: Config<AccountId = AccountId32, Proof = MultiSignature>,
        T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
        <<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<AccountId32>,
    }

    update_processor_pairings {
        let x in 1 .. T::MaxPairingUpdates::get();
        let mut updates = Vec::<ProcessorPairingUpdateFor<T>>::new();
        let caller: T::AccountId = alice_account_id().into();
        whitelist_account!(caller);
        for i in 0..x {
            updates.push(generate_pairing_update::<T>(ListUpdateOperation::Add, &caller));
        }
    }: _(RawOrigin::Signed(caller), updates.try_into().unwrap())

    update_processor_pairings_2 {
        let x in 1 .. T::MaxPairingUpdates::get();
        let mut updates_add = Vec::<ProcessorPairingUpdateFor<T>>::new();
        let caller: T::AccountId = alice_account_id().into();
        whitelist_account!(caller);
        for i in 0..x {
            updates_add.push(generate_pairing_update::<T>(ListUpdateOperation::Add, &caller));
        }
        Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), updates_add.clone().try_into().unwrap())?;
        let updates_remove = updates_add.into_iter().map(|update| ProcessorPairingUpdateFor::<T> {
            operation: ListUpdateOperation::Remove,
            item: ProcessorPairingFor::<T>::new(update.item.account),
        }).collect::<Vec<_>>();
    }: update_processor_pairings(RawOrigin::Signed(caller), updates_remove.try_into().unwrap())

    pair_with_manager {
        let (signer, manager_account) = generate_account();
        let (_, processor_account) = generate_account();
        let timestamp = 1657363915002u128;
        let message = [manager_account.encode(), timestamp.encode(), 1u128.encode()].concat();
        let signature: MultiSignature = signer.sign(&message).into();
        let item = ProcessorPairingFor::<T>::new_with_proof(manager_account, timestamp, signature);
    }: _(RawOrigin::Signed(processor_account), item)

    recover_funds {
        let caller: T::AccountId = alice_account_id().into();
        whitelist_account!(caller);
        let update = generate_pairing_update::<T>(ListUpdateOperation::Add, &caller);
        Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()].try_into().unwrap())?;
    }: _(RawOrigin::Signed(caller.clone()), update.item.account.into(), caller.clone().into())

    heartbeat {
        let caller: T::AccountId = alice_account_id().into();
    }: _(RawOrigin::Signed(caller.clone()))

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Test);
}
