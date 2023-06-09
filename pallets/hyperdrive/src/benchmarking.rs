use frame_benchmarking::benchmarks_instance_pallet;
use frame_benchmarking::whitelist_account;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_std::iter;

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
    l: usize,
    submit: bool,
) -> (T::AccountId, StateTransmitterUpdates<T>)
where
    T::AccountId: From<AccountId32>,
    T::BlockNumber: From<u64>,
{
    let caller: T::AccountId = alice_account_id().into();
    whitelist_account!(caller);

    let actions = StateTransmitterUpdates::<T>::try_from(
        iter::repeat(StateTransmitterUpdate::Add(
            caller.clone(),
            ActivityWindow {
                start_block: 0.into(),
                end_block: 100.into(),
            },
        ))
        .take(l)
        .collect::<Vec<StateTransmitterUpdateFor<T>>>(),
    )
    .unwrap();

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
        let l in 0 .. STATE_TRANSMITTER_UPDATES_MAX_LENGTH;

        // just create the data, do not submit the actual call (it gets executed by the benchmark call)
        let (caller, actions) = update_state_transmitters_helper::<T, I>(l as usize, false);
    }: _(RawOrigin::Root, actions.clone())
    verify {
        assert_last_event::<T, I>(Event::StateTransmittersUpdate{
                    added: iter::repeat((
                            alice_account_id().into(),
                            ActivityWindow {
                                start_block: 0.into(),
                                end_block: 100.into()
                            }
                        ))
                        .take(l as usize)
                        .collect::<Vec<(T::AccountId, ActivityWindow<<T as frame_system::Config>::BlockNumber>)>>(),
                    updated: vec![],
                    removed: vec![],
                }.into());
    }

    submit_state_merkle_root {
        // add the transmitters and submit before benchmarked extrinsic
        let (caller, _) = update_state_transmitters_helper::<T, I>(1, true);
    }: _(RawOrigin::Signed(caller.clone()), 1.into(), HASH.into())
    verify {
         assert_event::<T, I>(Event::StateMerkleRootSubmitted{
                    source: caller.clone(),
                    snapshot: 1.into(),
                    state_merkle_root: HASH.into()
                }.into());
    }

    impl_benchmark_test_suite!(AcurastHyperdrive, crate::mock::new_test_ext(), mock::Test);
}
