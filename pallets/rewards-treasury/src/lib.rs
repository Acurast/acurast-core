#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        log,
        pallet_prelude::*,
        traits::{tokens::fungible::Mutate, Get},
    };
    use frame_system::pallet_prelude::BlockNumberFor;
    use pallet_balances;
    use sp_std::prelude::*;

    use crate::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_balances::Config<I> {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The epoch length in blocks. At each epoch's end the penultimate (last but one) balance is burnt.
        type Epoch: Get<<Self as frame_system::Config>::BlockNumber>;
        /// The ID for this pallet
        #[pallet::constant]
        type Treasury: Get<<Self as frame_system::Config>::AccountId>;
    }

    #[pallet::storage]
    #[pallet::getter(fn penultimate_balance)]
    pub(super) type PenultimateBalance<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A vester started vesting. [amount_burnt]
        BurntFromTreasuryAtEndOfEpoch(T::Balance),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T, I = ()> {}

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_finalize(_: BlockNumberFor<T>) {}

        fn on_initialize(current_block: T::BlockNumber) -> Weight {
            if current_block % T::Epoch::get() == 0u16.into() {
                (match <PenultimateBalance<T, I>>::try_mutate(
                    |penultimate_balance| -> Result<T::Balance, DispatchError> {
                        let actual_burnt = pallet_balances::Pallet::<T, I>::burn_from(
                            &T::Treasury::get(),
                            penultimate_balance.to_owned(),
                        )?;

                        *penultimate_balance =
                            pallet_balances::Pallet::<T, I>::free_balance(T::Treasury::get());

                        Ok(actual_burnt)
                    },
                ) {
                    Ok(actual_burnt) => {
                        Self::deposit_event(Event::BurntFromTreasuryAtEndOfEpoch(actual_burnt));
                    }
                    Err(e) => {
                        log::error!(
                            target: "runtime::pallet_acurast_rewards_treasury",
                            "Error reducing treasury balance: {:?}",
                            e,
                        );
                    }
                });
                T::DbWeight::get().reads_writes(3, 1)
            } else {
                T::DbWeight::get().reads(0)
            }
        }
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {}
}
