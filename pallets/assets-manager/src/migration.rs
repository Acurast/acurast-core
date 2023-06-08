use frame_support::{traits::GetStorageVersion, weights::Weight};
use sp_core::Get;

use super::*;

pub fn migrate_to_v2<T: Config<I>, I: 'static>() -> Weight {
    let onchain_version = Pallet::<T, I>::on_chain_storage_version();
    if onchain_version < crate::STORAGE_VERSION {
        let mut count = 0u32;
        count += AssetIndex::<T, I>::clear(100, None).loops;
        count += ReverseAssetIndex::<T, I>::clear(100, None).loops;

        STORAGE_VERSION.put::<Pallet<T, I>>();
        T::DbWeight::get().reads_writes((count + 1).into(), (count + 1).into())
    } else {
        Weight::zero()
    }
}
