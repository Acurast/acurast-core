#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use core::ops::AddAssign;

use frame_support::{traits::Get, weights::Weight};
use sp_arithmetic::Percent;

pub use self::pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::{ValueQuery, *};
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;
        #[pallet::constant]
        type DefaultFeePercentage: Get<Percent>;
    }

    #[pallet::type_value]
    pub fn DefaultFeePercentage<T: Config>() -> Percent {
        T::DefaultFeePercentage::get()
    }

    #[pallet::storage]
    #[pallet::getter(fn fee_percentage)]
    pub type FeePercentage<T> =
        StorageMap<_, Blake2_128, u16, Percent, ValueQuery, DefaultFeePercentage<T>>;

    #[pallet::storage]
    #[pallet::getter(fn fee_version)]
    pub type Version<T> = StorageValue<_, u16, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event {
        FeeUpdated { version: u16, fee: Percent },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn update_fee_percentage(origin: OriginFor<T>, fee: Percent) -> DispatchResult {
            ensure_root(origin)?;
            let (new_version, _) = Self::set_fee_percentage(fee);
            Self::deposit_event(Event::FeeUpdated {
                version: new_version,
                fee,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn set_fee_percentage(fee: Percent) -> (u16, u64) {
        let new_version = <Version<T>>::mutate(|version| {
            version.add_assign(1);
            *version
        });
        <FeePercentage<T>>::set(new_version, fee);
        (new_version, T::DbWeight::get().write)
    }
}
