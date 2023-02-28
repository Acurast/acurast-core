use frame_benchmarking::benchmarks_instance_pallet;
use frame_benchmarking::whitelist_account;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_core::crypto::AccountId32;
use sp_core::H256;

pub use crate::stub::*;
use crate::types::*;
use crate::Pallet as AcurastHyperdrive;

use super::*;

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn assert_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

fn update_state_transmitters_helper<T: Config<I>, I: 'static>(
    submit: bool,
) -> (T::AccountId, Vec<StateTransmitterUpdateFor<T>>)
where
    T::AccountId: From<AccountId32>,
    T::BlockNumber: From<u64>,
{
    let caller: T::AccountId = alice_account_id().into();
    whitelist_account!(caller);

    let actions = vec![StateTransmitterUpdate::Add(
        caller.clone(),
        ActivityWindow {
            start_block: 0.into(),
            end_block: 100.into(),
        },
    )];

    if submit {
        let call = AcurastHyperdrive::<T, I>::update_state_transmitters(
            RawOrigin::Root.into(),
            actions.clone(),
        );
        assert_ok!(call);
    }

    (caller, actions)
}

benchmarks_instance_pallet! {
    where_clause {
        where
        T: Config<I>,
        T::AccountId: From<AccountId32>,
        T::BlockNumber: From<u64>,
        <T as pallet::Config<I>>::TargetChainBlockNumber: From<u64>,
        <T as pallet::Config<I>>::TargetChainHash: From<H256>,
    }
    update_state_transmitters {
        // just create the data, do not submit the actual call (we want to benchmark `advertise`)
        let (caller, actions) = update_state_transmitters_helper::<T, I>(false);
    }: _(RawOrigin::Root, actions.clone())
    verify {
        assert_last_event::<T, I>(Event::StateTransmittersUpdate{
                    added: vec![
                        (
                            alice_account_id().into(),
                            ActivityWindow {
                                start_block: 0.into(),
                                end_block: 100.into()
                            }
                        )
                    ],
                    updated: vec![],
                    removed: vec![],
                }.into());
    }

    submit_state_merkle_root {
        // create the data and submit so we have an add in storage to delete when benchmarking `delete_advertisement`
        let (caller, _) = update_state_transmitters_helper::<T, I>(true);
    }: _(RawOrigin::Signed(caller.clone()), 5.into(), HASH.into())
    verify {
         assert_event::<T, I>(Event::StateMerkleRootSubmitted{
                    block: 5.into(),
                    state_merkle_root: HASH.into()
                }.into());
    }

    impl_benchmark_test_suite!(AcurastHyperdrive, crate::mock::new_test_ext(), mock::Test);
}
