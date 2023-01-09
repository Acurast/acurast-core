#![cfg_attr(not(feature = "std"), no_std)]

// #[cfg(test)]
// mod mock;
//
// #[cfg(test)]
// mod tests;
//
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
pub mod weights;

use frame_support::{dispatch::Weight, traits::Get};
use sp_runtime::traits::StaticLookup;

pub use pallet::*;
pub use weights::WeightInfo;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

pub trait AssetBarrier<AccountId> {
    fn can_create_asset(who: AccountId) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use xcm::latest::MultiLocation;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_assets::Config<I> {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as pallet_assets::Config<I>>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Barrier: AssetBarrier<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn asset_index)]
    pub type AssetIndex<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, <T as pallet_assets::Config<I>>::AssetId, MultiLocation>;

    #[pallet::storage]
    #[pallet::getter(fn reverse_asset_index)]
    pub type ReverseAssetIndex<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, MultiLocation, <T as pallet_assets::Config<I>>::AssetId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {}

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The job registration's reward type is not supported.
        AssetAlreadyIndexed,
        IdAlreadyUsed,
        CreationNotAllowed,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I>
    where
        T: pallet_assets::Config<I>,
    {
        /// Creates and indexes a bijective mapping `id <-> native_id` and proxies to [`pallet_assets::Pallet::create()`].
        ///
        /// This extrinsic is idempotent when used with the same `id` and `asset` (does not receate the asset in `pallet_asset`.
        /// Trying to index an already indexed asset or using the same id to index a different asset results in an error.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::create())]
        pub fn create(
            origin: OriginFor<T>,
            id: <T as pallet_assets::Config<I>>::AssetIdParameter,
            asset: MultiLocation,
            admin: AccountIdLookupOf<T>,
            min_balance: <T as pallet_assets::Config<I>>::Balance,
        ) -> DispatchResult {
            let origin_ = ensure_signed(origin.clone())?;
            ensure!(T::Barrier::can_create_asset(origin_), Error::<T, I>::CreationNotAllowed);

            {
                let id: <T as pallet_assets::Config<I>>::AssetId = id.into();

                if let Some(value) = <AssetIndex<T, I>>::get(&id) {
                    ensure!(value == asset, Error::<T, I>::IdAlreadyUsed);
                    return Ok(());
                } else {
                    <AssetIndex<T, I>>::insert(&id, &asset);
                    if let Some(value) = <ReverseAssetIndex<T, I>>::get(&asset) {
                        ensure!(value == id, Error::<T, I>::AssetAlreadyIndexed);
                        return Ok(());
                    } else {
                        <ReverseAssetIndex<T, I>>::insert(&asset, &id);
                    }
                }
            }

            <pallet_assets::Pallet<T, I>>::create(origin, id, admin, min_balance)?;

            Ok(())
        }

        /// Proxies to [`pallet_assets::Pallet::set_metadata()`].
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::set_metadata(name.len() as u32, symbol.len() as u32))]
        pub fn set_metadata(
            origin: OriginFor<T>,
            id: T::AssetIdParameter,
            name: Vec<u8>,
            symbol: Vec<u8>,
            decimals: u8,
        ) -> DispatchResult {
            <pallet_assets::Pallet<T, I>>::set_metadata(origin, id, name, symbol, decimals)?;
            Ok(())
        }
    }
}
