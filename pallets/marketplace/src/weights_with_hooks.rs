use crate::weights::WeightInfo;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_acurast` including hooks from `pallet_marketplace`.
pub struct WeightInfoWithHooks<T>(PhantomData<T>);
impl<T: frame_system::Config + pallet_acurast::Config> pallet_acurast::WeightInfo
    for WeightInfoWithHooks<T>
{
    fn register() -> Weight {
        WeightInfo::<T>::register()
    }
    fn deregister() -> Weight {
        WeightInfo::<T>::deregister()
    }
    fn update_allowed_sources() -> Weight {
        <T as pallet_acurast::Config>::WeightInfo::update_allowed_sources()
    }
    fn update_job_assignments() -> Weight {
        <T as pallet_acurast::Config>::WeightInfo::update_job_assignments()
    }
    fn fulfill() -> Weight {
        WeightInfo::<T>::fulfill()
    }
    fn submit_attestation() -> Weight {
        <T as pallet_acurast::Config>::WeightInfo::submit_attestation()
    }
    fn update_certificate_revocation_list() -> Weight {
        <T as pallet_acurast::Config>::WeightInfo::update_certificate_revocation_list()
    }
}
