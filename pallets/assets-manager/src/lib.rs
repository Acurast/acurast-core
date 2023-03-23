#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod weights;

pub mod traits;

use sp_runtime::traits::StaticLookup;

pub use pallet::*;
use sp_std::borrow::Borrow;
use sp_std::prelude::*;
pub use weights::WeightInfo;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::AssetValidator;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use xcm::latest::MultiLocation;
    use xcm::prelude::{Abstract, AssetId, Concrete, GeneralIndex, PalletInstance, Parachain, X3};
    use xcm_executor::traits::Convert;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_assets::Config<I> {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as pallet_assets::Config<I>>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        /// Genesis assets: `internal asset ID -> asset ID` (Statemint's general index)
        // TODO generalize asset ID to any XCM AssetID once structs derive deserialize (merged with XCM-3)
        pub assets: Vec<(<T as pallet_assets::Config<I>>::AssetId, u32, u8, u128)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        fn default() -> Self {
            Self { assets: vec![] }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I> {
        fn build(&self) {
            for &(internal_asset_id, parachain, pallet_instance, general_index) in &self.assets {
                let asset_id = AssetId::Concrete(MultiLocation::new(
                    1,
                    X3(
                        Parachain(parachain),
                        PalletInstance(pallet_instance),
                        GeneralIndex(general_index),
                    ),
                ));
                assert!(
                    !<AssetIndex<T, I>>::contains_key(&internal_asset_id),
                    "Asset internal id already in use"
                );
                <AssetIndex<T, I>>::insert(&internal_asset_id, &asset_id);
                assert!(
                    !<ReverseAssetIndex<T, I>>::contains_key(&asset_id),
                    "Asset id already in use"
                );
                <ReverseAssetIndex<T, I>>::insert(&asset_id, &internal_asset_id);
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn asset_index)]
    pub type AssetIndex<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, <T as pallet_assets::Config<I>>::AssetId, AssetId>;

    #[pallet::storage]
    #[pallet::getter(fn reverse_asset_index)]
    pub type ReverseAssetIndex<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, AssetId, <T as pallet_assets::Config<I>>::AssetId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {}

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The job registration's reward type is not supported.
        AssetAlreadyIndexed,
        IdAlreadyUsed,
        CreationNotAllowed,
        AssetNotIndexed,
        InvalidAssetIndex,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I>
    where
        T: pallet_assets::Config<I>,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::create())]
        pub fn create(
            origin: OriginFor<T>,
            id: <T as pallet_assets::Config<I>>::AssetIdParameter,
            asset: AssetId,
            admin: AccountIdLookupOf<T>,
            min_balance: T::Balance,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin.clone())?;
            let new = Self::update_index(id, asset)?;

            if new {
                <pallet_assets::Pallet<T, I>>::create(origin, id, admin, min_balance)
            } else {
                Ok(())
            }
        }

        /// Creates and indexes a bijective mapping `id <-> internal_id` and proxies to [`pallet_assets::Pallet::force_create()`].
        ///
        /// This extrinsic is idempotent when used with the same `id` and `asset` (does not receate the asset in `pallet_asset`.
        /// Trying to index an already indexed asset or using the same id to index a different asset results in an error.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::create())]
        pub fn force_create(
            origin: OriginFor<T>,
            id: <T as pallet_assets::Config<I>>::AssetIdParameter,
            asset: AssetId,
            owner: AccountIdLookupOf<T>,
            is_sufficient: bool,
            min_balance: T::Balance,
        ) -> DispatchResult {
            T::ManagerOrigin::ensure_origin(origin.clone())?;
            let new = Self::update_index(id, asset)?;

            if new {
                <pallet_assets::Pallet<T, I>>::force_create(
                    origin,
                    id,
                    owner,
                    is_sufficient,
                    min_balance,
                )
            } else {
                Ok(())
            }
        }

        /// Proxies to [`pallet_assets::Pallet::set_metadata()`].
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::set_metadata(name.len() as u32, symbol.len() as u32))]
        pub fn set_metadata(
            origin: OriginFor<T>,
            id: AssetId,
            name: BoundedVec<u8, T::StringLimit>,
            symbol: BoundedVec<u8, T::StringLimit>,
            decimals: u8,
        ) -> DispatchResult {
            let id = <ReverseAssetIndex<T, I>>::get(&id).ok_or(Error::<T, I>::AssetNotIndexed)?;
            <pallet_assets::Pallet<T, I>>::set_metadata(
                origin,
                id.into(),
                name.into(),
                symbol.into(),
                decimals,
            )
        }

        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            id: AssetId,
            target: AccountIdLookupOf<T>,
            #[pallet::compact] amount: T::Balance,
        ) -> DispatchResult {
            let id = <ReverseAssetIndex<T, I>>::get(&id).ok_or(Error::<T, I>::AssetNotIndexed)?;
            <pallet_assets::Pallet<T, I>>::transfer(origin, id.into(), target, amount)
        }

        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::force_transfer())]
        pub fn force_transfer(
            origin: OriginFor<T>,
            id: AssetId,
            source: AccountIdLookupOf<T>,
            dest: AccountIdLookupOf<T>,
            #[pallet::compact] amount: T::Balance,
        ) -> DispatchResult {
            let id = <ReverseAssetIndex<T, I>>::get(&id).ok_or(Error::<T, I>::AssetNotIndexed)?;
            <pallet_assets::Pallet<T, I>>::force_transfer(origin, id.into(), source, dest, amount)
        }
    }

    impl<T: Config<I> + pallet_assets::Config<I>, I: 'static> Pallet<T, I> {
        fn update_index(
            id: <T as pallet_assets::Config<I>>::AssetIdParameter,
            asset: AssetId,
        ) -> Result<bool, DispatchError> {
            let id: <T as pallet_assets::Config<I>>::AssetId = id.into();

            if let Some(value) = <AssetIndex<T, I>>::get(&id) {
                ensure!(value == asset, Error::<T, I>::IdAlreadyUsed);
                return Ok(false);
            } else {
                <AssetIndex<T, I>>::insert(&id, &asset);
                if let Some(value) = <ReverseAssetIndex<T, I>>::get(&asset) {
                    ensure!(value == id, Error::<T, I>::AssetAlreadyIndexed);
                    return Ok(false);
                } else {
                    <ReverseAssetIndex<T, I>>::insert(&asset, &id);
                }
            }

            Ok(true)
        }
    }

    impl<T: Config<I> + pallet_assets::Config<I>, I: 'static> AssetValidator<AssetId> for Pallet<T, I> {
        type Error = Error<T, I>;

        fn validate(asset: &AssetId) -> Result<(), Self::Error> {
            Self::reverse_asset_index(asset).ok_or(Error::<T, I>::AssetNotIndexed)?;
            Ok(())
        }
    }

    impl<T: Config<I> + pallet_assets::Config<I>, I: 'static> Convert<MultiLocation, T::AssetId>
        for Pallet<T, I>
    {
        fn convert_ref(id: impl Borrow<MultiLocation>) -> Result<T::AssetId, ()> {
            Ok(Self::reverse_asset_index(Concrete(id.borrow().clone())).ok_or(())?)
        }
        fn reverse_ref(id: impl Borrow<T::AssetId>) -> Result<MultiLocation, ()> {
            match Self::asset_index(id.borrow()).ok_or(())? {
                Concrete(location) => Ok(location),
                Abstract(_) => Err(()),
            }
        }
    }
}
