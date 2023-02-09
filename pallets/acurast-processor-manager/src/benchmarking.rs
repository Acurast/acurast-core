//! Benchmarking setup for pallet-acurast-processor-manager

use crate::stub::{alice_account_id, generate_account};

use super::*;

use acurast_common::ListUpdateOperation;
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

fn generate_pairing_update<T: Config<AccountId = AccountId32, Proof = MultiSignature>>(
    operation: ListUpdateOperation,
) -> ProcessorPairingUpdateFor<T> {
    let (processor_pair, processor_account_id) = generate_account();
    let message = vec![0u8];
    let signature: MultiSignature = processor_pair.sign(&message).into();
    ProcessorPairingUpdateFor::<T> {
        operation,
        item: ProcessorPairingFor::<T>::new_with_proof(
            processor_account_id,
            message.try_into().unwrap(),
            signature,
        ),
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
            updates.push(generate_pairing_update::<T>(ListUpdateOperation::Add));
        }
    }: _(RawOrigin::Signed(caller), updates)

    update_processor_pairings_2 {
        let x in 1 .. T::MaxPairingUpdates::get();
        let mut updates_add = Vec::<ProcessorPairingUpdateFor<T>>::new();
        let caller: T::AccountId = alice_account_id().into();
        whitelist_account!(caller);
        for i in 0..x {
            updates_add.push(generate_pairing_update::<T>(ListUpdateOperation::Add));
        }
        Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), updates_add.clone())?;
        let updates_remove = updates_add.into_iter().map(|update| ProcessorPairingUpdateFor::<T> {
            operation: ListUpdateOperation::Remove,
            item: ProcessorPairingFor::<T>::new(update.item.processor),
        }).collect::<Vec<_>>();
    }: update_processor_pairings(RawOrigin::Signed(caller), updates_remove)

    recover_funds {
        let caller: T::AccountId = alice_account_id().into();
        whitelist_account!(caller);
        let update = generate_pairing_update::<T>(ListUpdateOperation::Add);
        Pallet::<T>::update_processor_pairings(RawOrigin::Signed(caller.clone()).into(), vec![update.clone()])?;
    }: _(RawOrigin::Signed(caller.clone()), update.item.processor.into(), caller.clone().into())

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Test);
}
